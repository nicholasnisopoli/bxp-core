// src/client.rs
use crate::bxp_capnp; // Import from the crate root
use crate::protocol::{Action, BxpResponse};
use anyhow::Result;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream};
use std::{net::SocketAddr, sync::Arc};
// Removed unused tokio::io::AsyncWriteExt

pub struct BxpClient;

impl BxpClient {
    pub async fn connect(addr: &str) -> Result<BxpClientConnection> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let socket_addr: SocketAddr = addr.parse()?;

        let client_crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertVerification))
            .with_no_client_auth();
            
        let mut client_crypto_config = client_crypto;
        client_crypto_config.alpn_protocols = vec![b"bxp/1".to_vec()];

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        endpoint.set_default_client_config(ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto_config)
                .map_err(|e| anyhow::anyhow!("{:?}", e))?,
        )));

        let quic_conn = endpoint.connect(socket_addr, "localhost")?.await?;
        
        let (control_send, control_recv) = quic_conn.open_bi().await?;

        Ok(BxpClientConnection {
            quic_conn,
            control_send,
            control_recv,
            endpoint,
        })
    }
}

pub struct BxpClientConnection {
    quic_conn: Connection,
    control_send: SendStream,
    control_recv: RecvStream,
    endpoint: Endpoint,
}

impl BxpClientConnection {
    pub async fn send_request(&mut self, req_id: u32, action: Action, uri: &str) -> Result<()> {
        let mut message = capnp::message::Builder::new_default();
        {
            let mut request = message.init_root::<bxp_capnp::request::Builder>();
            request.set_request_id(req_id);
            request.set_action(action);
            request.set_resource_uri(uri);
        }

        let mut out_buf = Vec::new();
        capnp::serialize::write_message(&mut out_buf, &message)?;
        self.control_send.write_all(&(out_buf.len() as u32).to_le_bytes()).await?;
        self.control_send.write_all(&out_buf).await?;
        Ok(())
    }

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
        Ok(BxpResponse {
            req_id: response.get_request_id(),
            status_code: response.get_status_code(),
        })
    }

    pub async fn send_data_stream(&self, data: &[u8]) -> Result<()> {
        let mut data_send = self.quic_conn.open_uni().await?;
        data_send.write_all(data).await?;
        data_send.finish()?; 
        Ok(())
    }

    pub async fn read_data_stream(&self) -> Result<Vec<u8>> {
        let mut data_recv = self.quic_conn.accept_uni().await?;
        let payload = data_recv.read_to_end(1024 * 1024 * 100).await?;
        Ok(payload)
    }

    pub async fn close(self) {
        self.quic_conn.close(0u32.into(), b"Done");
        self.endpoint.wait_idle().await;
    }
}

// --- Dummy Cert Verifier for Local Testing ---
#[derive(Debug)]
struct NoCertVerification;

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
        &self, _message: &[u8], _cert: &rustls::pki_types::CertificateDer<'_>, _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    
    fn verify_tls13_signature(
        &self, _message: &[u8], _cert: &rustls::pki_types::CertificateDer<'_>, _dss: &rustls::DigitallySignedStruct,
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