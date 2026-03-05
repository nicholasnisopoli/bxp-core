use anyhow::Result;
use quinn::{ClientConfig, Endpoint, ServerConfig};
use std::{net::SocketAddr, sync::Arc};

// Include the auto-generated Cap'n Proto Rust code
pub mod bxp_capnp {
    include!(concat!(env!("OUT_DIR"), "/bxp_capnp.rs"));
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Fix for rustls 0.23+: Explicitly install the 'ring' crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();

    let server_addr: SocketAddr = "127.0.0.1:4433".parse()?;

    // 2. Generate dummy TLS certificates for local QUIC testing
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = cert.cert.der().to_vec();
    let priv_key = cert.key_pair.serialize_der();

    // 3. Configure Rustls Server Config (Set ALPN here!)
    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(cert_der.clone())],
            rustls::pki_types::PrivateKeyDer::try_from(priv_key)
                .map_err(|e| anyhow::anyhow!("{:?}", e))?,
        )?;
    server_crypto.alpn_protocols = vec![b"bxp/1".to_vec()];

    // 4. Wrap rustls config in Quinn's QuicServerConfig
    let server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)
            .map_err(|e| anyhow::anyhow!("{:?}", e))?,
    ));

    // 5. Start the Server
    let endpoint = Endpoint::server(server_config, server_addr)?;
    println!("[Server] BXP Node listening on {}", server_addr);

    tokio::spawn(async move {
        run_server(endpoint).await.unwrap();
    });

    // 6. Start the Client (Wait a moment for server to boot)
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    run_client(server_addr, cert_der).await?;

    Ok(())
}

/// ==========================================
/// BXP SERVER IMPLEMENTATION
/// ==========================================
async fn run_server(endpoint: Endpoint) -> Result<()> {
    while let Some(incoming) = endpoint.accept().await {
        // Spawn a concurrent Tokio task for every new client
        tokio::spawn(async move {
            let connection = match incoming.await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("[Server] Connection failed: {}", e);
                    return;
                }
            };

            println!("[Server] Client connected: {}", connection.remote_address());

            // Handle the stream logic
            if let Err(e) = handle_client(&connection).await {
                eprintln!("[Server] Client error: {:?}", e);
            }

            // Keep the connection alive until the client gracefully disconnects!
            let _ = connection.closed().await;
            println!("[Server] Connection gracefully closed.");
        });
    }
    Ok(())
}

async fn handle_client(connection: &quinn::Connection) -> Result<()> {
    // Accept Stream 0 (Bidirectional Control Stream)
    let (mut control_send, mut control_recv) = connection.accept_bi().await?;

    // Read the Cap'n proto message length (4 bytes), then the buffer
    let mut len_buf = [0u8; 4];
    control_recv.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut msg_buf = vec![0u8; len];
    control_recv.read_exact(&mut msg_buf).await?;

    // Deserialize zero-copy Cap'n Proto
    let message_reader = capnp::serialize::read_message(
        &mut msg_buf.as_slice(),
        capnp::message::ReaderOptions::new(),
    )?;
    let request = message_reader.get_root::<crate::bxp_capnp::request::Reader>()?;

    let req_id = request.get_request_id();
    let uri = request.get_resource_uri()?.to_string()?;
    println!("[Server] Received Request on Stream 0 | ID: {}, URI: {}", req_id, uri);

    // Open a new Unidirectional stream (Data Stream) for the payload
    let mut data_send = connection.open_uni().await?;
    println!("[Server] Opened Data Stream: {}", data_send.id());

    // Send Response metadata back on Stream 0
    let mut message = capnp::message::Builder::new_default();
    {
        let mut response = message.init_root::<crate::bxp_capnp::response::Builder>();
        response.set_request_id(req_id);
        response.set_status_code(0x01); // Success
    }
    let mut out_buf = Vec::new();
    capnp::serialize::write_message(&mut out_buf, &message)?;

    control_send.write_all(&(out_buf.len() as u32).to_le_bytes()).await?;
    control_send.write_all(&out_buf).await?;

    // Blast the raw binary data on the Data Stream
    let dummy_payload = b"This is the raw, zero-copy binary payload of the requested file.";
    data_send.write_all(dummy_payload).await?;
    data_send.finish()?; // Send QUIC FIN bit
    println!("[Server] Payload transferred. Data Stream closed.");
    
    Ok(())
}

/// ==========================================
/// BXP CLIENT IMPLEMENTATION
/// ==========================================
async fn run_client(server_addr: SocketAddr, server_cert: Vec<u8>) -> Result<()> {
    // Client TLS setup accepting our dummy cert
    let mut roots = rustls::RootCertStore::empty();
    roots.add(rustls::pki_types::CertificateDer::from(server_cert))?;
    let mut client_crypto = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    client_crypto.alpn_protocols = vec![b"bxp/1".to_vec()]; // ALPN Negotiation

    // Wrap rustls config in Quinn's QuicClientConfig
    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
            .map_err(|e| anyhow::anyhow!("{:?}", e))?,
    )));

    println!("[Client] Connecting to BXP Server...");
    let connection = endpoint.connect(server_addr, "localhost")?.await?;

    // Open Stream 0 (Bidirectional Control Stream)
    let (mut control_send, mut control_recv) = connection.open_bi().await?;
    println!("[Client] Opened Stream 0 (Control). Sending Fetch Request...");

    // Build Cap'n Proto Request
    let mut message = capnp::message::Builder::new_default();
    {
        let mut request = message.init_root::<crate::bxp_capnp::request::Builder>();
        request.set_request_id(42);
        request.set_action(crate::bxp_capnp::Action::Fetch); 
        request.set_resource_uri("bxp://localhost/assets/data.bin");
    }

    let mut out_buf = Vec::new();
    capnp::serialize::write_message(&mut out_buf, &message)?;
    control_send.write_all(&(out_buf.len() as u32).to_le_bytes()).await?;
    control_send.write_all(&out_buf).await?;

    // Read Response on Stream 0
    let mut len_buf = [0u8; 4];
    control_recv.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut msg_buf = vec![0u8; len];
    control_recv.read_exact(&mut msg_buf).await?;

    let message_reader = capnp::serialize::read_message(
        &mut msg_buf.as_slice(),
        capnp::message::ReaderOptions::new(),
    )?;
    let response = message_reader.get_root::<crate::bxp_capnp::response::Reader>()?;
    println!("[Client] Received Control Response | Status: 0x{:02X}", response.get_status_code());

    // Accept the Unidirectional Data Stream containing the payload
    let mut data_recv = connection.accept_uni().await?;
    println!("[Client] Accepted Data Stream: {}", data_recv.id());

    // Use Quinn's built-in read_to_end with a max size limit (e.g., 10MB) to prevent memory exhaustion
    let payload = data_recv.read_to_end(1024 * 1024 * 10).await?;
    println!("[Client] Payload received successfully ({} bytes).", payload.len());
    println!("[Client] Content: {}", String::from_utf8_lossy(&payload));

    // Gracefully hang up the QUIC connection!
    connection.close(0u32.into(), b"Operation Complete");
    endpoint.wait_idle().await;

    Ok(())
}