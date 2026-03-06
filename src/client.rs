// src/client.rs
use crate::bxp_capnp;
use crate::protocol::{BxpAction, BxpResponse};
use anyhow::Result;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, TransportConfig};
use std::{net::SocketAddr, sync::Arc, time::Duration};

pub struct BxpClient;

impl BxpClient {
    /// Connect to a BXP server.
    ///
    /// `addr` is the socket address to connect to (e.g. `"1.2.3.4:9000"`).
    /// `server_name` is the hostname used for TLS SNI and certificate
    /// verification (e.g. `"bxp.example.com"`). It must match the name on the
    /// server's certificate.
    ///
    /// # TLS modes
    ///
    /// - **Default (production):** verifies the server certificate against the
    ///   Mozilla root CA bundle via `webpki-roots`. The connection is rejected
    ///   if the cert is invalid, expired, or from an unknown CA.
    ///
    /// - **`tls-insecure` feature:** skips all verification. Suitable only for
    ///   local development with self-signed certs. Enable with:
    ///   ```
    ///   cargo run --features tls-insecure
    ///   ```
    ///   This feature must never be enabled in production builds.
    pub async fn connect(addr: &str, server_name: &str) -> Result<BxpClientConnection> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let socket_addr: SocketAddr = addr.parse()?;

        let client_crypto = build_tls_config()?;
        let mut client_crypto_config = client_crypto;
        client_crypto_config.alpn_protocols = vec![b"bxp/1".to_vec()];

        let mut transport_config = TransportConfig::default();
        transport_config.keep_alive_interval(Some(Duration::from_secs(10)));
        transport_config.stream_receive_window(100_000_000u32.into());

        let quic_client_config =
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto_config)
                .map_err(|e| anyhow::anyhow!("{:?}", e))?;

        let mut client_config = ClientConfig::new(Arc::new(quic_client_config));
        client_config.transport_config(Arc::new(transport_config));

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        endpoint.set_default_client_config(client_config);

        let quic_conn = endpoint.connect(socket_addr, server_name)?.await?;
        let (control_send, control_recv) = quic_conn.open_bi().await?;

        Ok(BxpClientConnection {
            quic_conn,
            control_send,
            control_recv,
            endpoint,
        })
    }
}

#[cfg(not(feature = "tls-insecure"))]
fn build_tls_config() -> Result<rustls::ClientConfig> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let verifier = rustls::client::WebPkiServerVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build TLS verifier: {e}"))?;
    Ok(rustls::ClientConfig::builder()
        .with_webpki_verifier(verifier)
        .with_no_client_auth())
}

#[cfg(feature = "tls-insecure")]
fn build_tls_config() -> Result<rustls::ClientConfig> {
    Ok(rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertVerification))
        .with_no_client_auth())
}

pub struct BxpClientConnection {
    quic_conn: Connection,
    control_send: SendStream,
    control_recv: RecvStream,
    endpoint: Endpoint,
}

/// BXP client connection methods for sending requests, receiving responses, and streaming data.
impl BxpClientConnection {
    /// Sends a BXP request with the given action and URI, and waits for the response.
    /// The `req_id` is an opaque identifier chosen by the client to correlate requests and responses.
    /// The request is serialized using Cap'n Proto and sent over the control stream. The first 4 bytes of the message are a little-endian u32 indicating the payload length, followed by the Cap'n Proto message itself.
    pub async fn send_request(
        &mut self,
        req_id: u32,
        action: BxpAction,
        uri: &str,
    ) -> Result<()> {
        let mut message = capnp::message::Builder::new_default();
        {
            let mut request = message.init_root::<bxp_capnp::request::Builder>();
            request.set_request_id(req_id);
            request.set_action(action.into());
            request.set_resource_uri(uri);
        }
        let mut out_buf = vec![0u8; 4];
        capnp::serialize::write_message(&mut out_buf, &message)?;
        // Calculate the actual payload length and overwrite the first 4 bytes
        let payload_len = (out_buf.len() - 4) as u32;
        out_buf[0..4].copy_from_slice(&payload_len.to_le_bytes());
        
        self.control_send.write_all(&out_buf).await?;
        Ok(())
    }

    /// Reads the BXP response and parses the strict Status Code.
    /// The `req_id` is included in the response for correlation but is not validated by the client (the client can choose to ignore it or use it to match responses to requests).
    pub async fn receive_response(&mut self) -> Result<BxpResponse> {
        let mut len_buf = [0u8; 4];
        self.control_recv.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;

        let mut msg_buf = vec![0u8; len];
        self.control_recv.read_exact(&mut msg_buf).await?;

        let message_reader = capnp::serialize::read_message(
            &mut msg_buf.as_slice(),
            capnp::message::ReaderOptions::new(),
        )?;

        let response = message_reader.get_root::<bxp_capnp::response::Reader>()?;

        let capnp_status = response
            .get_status_code()
            .map_err(|_| anyhow::anyhow!("Unknown Status Code"))?;

        Ok(BxpResponse {
            req_id: response.get_request_id(),
            status: capnp_status.try_into()?,
        })
    }

    /// Sends a unidirectional data stream to the server, reading from any
    /// `AsyncRead` source.
    ///
    /// Writes the 4-byte `req_id` preamble, then streams `reader` directly
    /// into the QUIC send stream via `tokio::io::copy` — no intermediate
    /// buffer is allocated regardless of payload size. The source can be a
    /// `tokio::fs::File`, a `BufReader`, a decompressor pipeline, a
    /// `Cursor<&[u8]>` for in-memory data, or any other `AsyncRead`.
    ///
    /// # Example
    /// ```
    /// // Stream a file from disk:
    /// let file = tokio::fs::File::open("upload.bin").await?;
    /// client.send_data_stream(req_id, file).await?;
    ///
    /// // Send in-memory bytes:
    /// client.send_data_stream(req_id, std::io::Cursor::new(b"hello")).await?;
    /// ```
    pub async fn send_data_stream<R>(&self, req_id: u32, mut reader: R) -> Result<()>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut data_send = self.quic_conn.open_uni().await?;
        data_send.write_all(&req_id.to_le_bytes()).await?;
        tokio::io::copy(&mut reader, &mut data_send).await?;
        data_send.finish()?;
        Ok(())
    }

    /// Accepts an incoming unidirectional data stream from the server.
    ///
    /// Returns the req_id preamble and the live stream for incremental reading.
    pub async fn read_data_stream(&self) -> Result<(u32, RecvStream)> {
        let mut data_recv = self.quic_conn.accept_uni().await?;

        let mut id_buf = [0u8; 4];
        data_recv.read_exact(&mut id_buf).await?;
        let req_id = u32::from_le_bytes(id_buf);

        Ok((req_id, data_recv))
    }
    /// Closes the client connection gracefully.
    pub async fn close(self) {
        self.quic_conn.close(0u32.into(), b"Done");
        self.endpoint.wait_idle().await;
    }
}

// Compiled out entirely in production builds.
#[cfg(feature = "tls-insecure")]
#[derive(Debug)]
struct NoCertVerification;

#[cfg(feature = "tls-insecure")]
impl rustls::client::danger::ServerCertVerifier for NoCertVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}