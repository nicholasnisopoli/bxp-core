// src/server.rs
use crate::bxp_capnp;
use crate::protocol::{BxpRequest, BxpStatus};
use anyhow::Result;
use quinn::{Connection, Endpoint, RecvStream, SendStream, ServerConfig, TransportConfig};
use std::{net::SocketAddr, sync::Arc, time::Duration};

/// Maximum allowed size of a single control-channel message (request/response).
/// A client sending a length header larger than this is either buggy or malicious.
/// Prevents a single bad client from forcing a multi-GB allocation.
const MAX_CONTROL_MSG: usize = 1024 * 1024; // 1 MB
/// The BxpServer struct represents a BXP server that listens for incoming connections and handles requests. It encapsulates the QUIC endpoint and provides methods for accepting connections and managing them. The BxpServerConnection struct represents an active connection to a client, allowing the server to receive requests, send responses, and manage data streams. These structs are designed to provide a clean and ergonomic API for building BXP servers that can handle multiple clients concurrently while maintaining type safety and efficient resource management.
pub struct BxpServer {
    endpoint: Endpoint,
}

impl BxpServer {
    /// Bind a BXP server to `addr`.
    ///
    /// # TLS modes
    ///
    /// - **Default (production):** loads a PEM-encoded certificate chain and
    ///   private key from `cert_path` / `key_path`. The files must contain
    ///   valid, CA-signed material that clients can verify.
    ///
    /// - **`tls-insecure` feature:** `cert_path` and `key_path` are ignored.
    ///   An ephemeral self-signed certificate is generated at startup. Suitable
    ///   only for local development against a client built with the same flag.
    ///   Enable with:
    ///   ```
    ///   cargo run --features tls-insecure
    ///   ```
    pub async fn bind(addr: &str, cert_path: &str, key_path: &str) -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let socket_addr: SocketAddr = addr.parse()?;

        let mut server_crypto = build_server_tls(cert_path, key_path)?;
        server_crypto.alpn_protocols = vec![b"bxp/1".to_vec()];

        let mut transport_config = TransportConfig::default();
        transport_config.keep_alive_interval(Some(Duration::from_secs(10)));
        transport_config.stream_receive_window(100_000_000u32.into());

        let mut server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)
                .map_err(|e| anyhow::anyhow!("{:?}", e))?,
        ));
        server_config.transport_config(Arc::new(transport_config));

        let endpoint = Endpoint::server(server_config, socket_addr)?;
        Ok(Self { endpoint })
    }

    /// Returns the local address the server is actually bound to.
    ///
    /// Useful when binding to port 0 (OS-assigned port), e.g. in tests:
    /// ```
    /// let server = BxpServer::bind("127.0.0.1:0", "", "").await?;
    /// let addr = server.local_addr()?;
    /// ```
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.endpoint
            .local_addr()
            .map_err(|e| anyhow::anyhow!("local_addr: {e}"))
    }

    /// Accept the next incoming connection.
    ///
    /// Errors during the QUIC handshake or the initial control stream setup are
    /// logged as warnings and cause `None` to be returned so the caller's accept
    /// loop can continue cleanly. The connection is explicitly closed with an
    /// error code before being dropped so the remote side is not left hanging.
    pub async fn accept(&self) -> Option<BxpServerConnection> {
        let incoming = self.endpoint.accept().await?;

        let quic_conn = match incoming.await {
            Ok(c) => c,
            Err(_) => {
                return None;
            }
        };

        match quic_conn.accept_bi().await {
            Ok((control_send, control_recv)) => Some(BxpServerConnection {
                quic_conn,
                control_send,
                control_recv,
            }),
            Err(_) => {
                quic_conn.close(1u32.into(), b"control stream failed");
                None
            }
        }
    }
}

/// Builds the rustls `ServerConfig` for the active TLS mode.
#[cfg(not(feature = "tls-insecure"))]
fn build_server_tls(cert_path: &str, key_path: &str) -> Result<rustls::ServerConfig> {
    use std::fs;

    let cert_pem = fs::read(cert_path)
        .map_err(|e| anyhow::anyhow!("Failed to read cert file '{cert_path}': {e}"))?;
    let key_pem = fs::read(key_path)
        .map_err(|e| anyhow::anyhow!("Failed to read key file '{key_path}': {e}"))?;

    let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
        rustls_pemfile::certs(&mut cert_pem.as_slice())
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate PEM: {e}"))?;

    if certs.is_empty() {
        anyhow::bail!("No certificates found in '{cert_path}'");
    }

    let key = rustls_pemfile::private_key(&mut key_pem.as_slice())
        .map_err(|e| anyhow::anyhow!("Failed to parse private key PEM: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("No private key found in '{key_path}'"))?;

    Ok(rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?)
}

#[cfg(feature = "tls-insecure")]
fn build_server_tls(_cert_path: &str, _key_path: &str) -> Result<rustls::ServerConfig> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec());
    let priv_key = rustls::pki_types::PrivateKeyDer::try_from(cert.key_pair.serialize_der())
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    Ok(rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], priv_key)?)
}

pub struct BxpServerConnection {
    quic_conn: Connection,
    control_send: SendStream,
    control_recv: RecvStream,
}

impl BxpServerConnection {
    /// Receives the next request on the control stream.
    pub async fn receive_request(&mut self) -> Result<BxpRequest> {
        let mut len_buf = [0u8; 4];
        self.control_recv.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;

        if len > MAX_CONTROL_MSG {
            anyhow::bail!("Control message too large: {len} bytes (max {MAX_CONTROL_MSG})");
        }

        let mut msg_buf = vec![0u8; len];
        self.control_recv.read_exact(&mut msg_buf).await?;

        let message_reader = capnp::serialize::read_message(
            &mut msg_buf.as_slice(),
            capnp::message::ReaderOptions::new(),
        )?;

        let request = message_reader.get_root::<bxp_capnp::request::Reader>()?;
        let req_id = request.get_request_id();

        let capnp_action = request
            .get_action()
            .map_err(|_| anyhow::anyhow!("Invalid Action"))?;

        Ok(BxpRequest {
            req_id,
            action: capnp_action.try_into()?,
            uri: request.get_resource_uri()?.to_string()?,
        })
    }

    /// Sends a strictly typed BxpStatus code back to the client.
    pub async fn send_response(&mut self, req_id: u32, status: BxpStatus) -> Result<()> {
        let mut message = capnp::message::Builder::new_default();
        {
            let mut response = message.init_root::<bxp_capnp::response::Builder>();
            response.set_request_id(req_id);
            response.set_status_code(status.into());
        }
        // Pre-allocate the vector
        let mut out_buf = vec![0u8; 4];
        capnp::serialize::write_message(&mut out_buf, &message)?;
        // Calculate the actual payload length and overwrite the first 4 bytes
        let payload_len = (out_buf.len() - 4) as u32;
        out_buf[0..4].copy_from_slice(&payload_len.to_le_bytes());
        
        self.control_send.write_all(&out_buf).await?;
        Ok(())
    }

    /// Accepts an incoming unidirectional data stream from the client.
    ///
    /// Reads the 4-byte `req_id` preamble, then returns the live stream for
    /// the caller to consume incrementally.
    pub async fn read_data_stream(&self) -> Result<(u32, RecvStream)> {
        let mut data_recv = self.quic_conn.accept_uni().await?;

        let mut id_buf = [0u8; 4];
        data_recv.read_exact(&mut id_buf).await?;
        let req_id = u32::from_le_bytes(id_buf);

        Ok((req_id, data_recv))
    }

    /// Sends a unidirectional data stream to the client, reading from any
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
    /// let file = tokio::fs::File::open("large.bin").await?;
    /// conn.send_data_stream(req_id, file).await?;
    ///
    /// // Send in-memory bytes:
    /// conn.send_data_stream(req_id, std::io::Cursor::new(b"hello")).await?;
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

    /// Keeps the connection alive until the client gracefully disconnects.
    pub async fn wait_for_close(&self) {
        let _ = self.quic_conn.closed().await;
    }
}