// src/server.rs
use crate::bxp_capnp; // Import from the crate root
use crate::protocol::BxpRequest;
use anyhow::Result; // Removed unused Context
use quinn::{Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use std::{net::SocketAddr, sync::Arc};
// Removed unused tokio::io::AsyncWriteExt

pub struct BxpServer {
    endpoint: Endpoint,
}

impl BxpServer {
    pub async fn bind(addr: &str) -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let socket_addr: SocketAddr = addr.parse()?;

        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
        let cert_der = cert.cert.der().to_vec();
        let priv_key = cert.key_pair.serialize_der();

        let mut server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![rustls::pki_types::CertificateDer::from(cert_der.clone())],
                rustls::pki_types::PrivateKeyDer::try_from(priv_key)
                    .map_err(|e| anyhow::anyhow!("{:?}", e))?,
            )?;
        server_crypto.alpn_protocols = vec![b"bxp/1".to_vec()];

        let server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)
                .map_err(|e| anyhow::anyhow!("{:?}", e))?,
        ));

        let endpoint = Endpoint::server(server_config, socket_addr)?;
        Ok(Self { endpoint })
    }

    pub async fn accept(&self) -> Option<BxpServerConnection> {
        let incoming = self.endpoint.accept().await?;
        let quic_conn = incoming.await.ok()?;
        
        if let Ok((control_send, control_recv)) = quic_conn.accept_bi().await {
            Some(BxpServerConnection {
                quic_conn,
                control_send,
                control_recv,
                last_req_id: 0,
            })
        } else {
            None
        }
    }
}

pub struct BxpServerConnection {
    quic_conn: Connection,
    control_send: SendStream,
    control_recv: RecvStream,
    last_req_id: u32,
}

impl BxpServerConnection {
    pub async fn receive_request(&mut self) -> Result<BxpRequest> {
        let mut len_buf = [0u8; 4];
        self.control_recv.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;
        
        let mut msg_buf = vec![0u8; len];
        self.control_recv.read_exact(&mut msg_buf).await?;

        let message_reader = capnp::serialize::read_message(
            &mut msg_buf.as_slice(),
            capnp::message::ReaderOptions::new(),
        )?;
        
        let request = message_reader.get_root::<bxp_capnp::request::Reader>()?;
        self.last_req_id = request.get_request_id();

        Ok(BxpRequest {
            req_id: self.last_req_id,
            action: request.get_action().unwrap_or(bxp_capnp::Action::Fetch),
            uri: request.get_resource_uri()?.to_string()?,
        })
    }

    pub async fn send_response(&mut self, status_code: u16) -> Result<()> {
        let mut message = capnp::message::Builder::new_default();
        {
            let mut response = message.init_root::<bxp_capnp::response::Builder>();
            response.set_request_id(self.last_req_id);
            response.set_status_code(status_code);
        }
        
        let mut out_buf = Vec::new();
        capnp::serialize::write_message(&mut out_buf, &message)?;
        self.control_send.write_all(&(out_buf.len() as u32).to_le_bytes()).await?;
        self.control_send.write_all(&out_buf).await?;
        Ok(())
    }

    pub async fn read_data_stream(&self) -> Result<Vec<u8>> {
        let mut data_recv = self.quic_conn.accept_uni().await?;
        let payload = data_recv.read_to_end(1024 * 1024 * 100).await?; 
        Ok(payload)
    }

    pub async fn send_data_stream(&self, data: &[u8]) -> Result<()> {
        let mut data_send = self.quic_conn.open_uni().await?;
        data_send.write_all(data).await?;
        data_send.finish()?; 
        Ok(())
    }

    /// Keeps the connection alive until the client gracefully disconnects
    pub async fn wait_for_close(&self) {
        let _ = self.quic_conn.closed().await;
    }
}