// examples/file_transfer.rs

use anyhow::Result;
use bxp_core::{
    BxpAction, BxpClient, BxpRequest, BxpRouter, BxpServer, BxpServerConnection, BxpStatus,
};
use std::sync::Arc;
use tokio::time::Duration;

// --- Clean, isolated handler function ---
async fn handle_fetch(req: BxpRequest, conn: &mut BxpServerConnection) -> Result<()> {
    println!("🟢 [Server] Client requested file: {}", req.uri);
    conn.send_response(req.req_id, BxpStatus::Success).await?;
    
    let dummy_file = std::io::Cursor::new(b"Imagine this is a 5GB video file chunked over the network...");
    
    println!("🟢 [Server] Streaming payload...");
    conn.send_data_stream(req.req_id, dummy_file).await?;
    
    println!("🟢 [Server] Transfer complete.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting BXP Streaming Demo...\n");

    let router = BxpRouter::new().route(BxpAction::Fetch, "bxp://assets/movie.mp4", handle_fetch);
    let shared_router = Arc::new(router);
    let server = BxpServer::bind("127.0.0.1:4434", "", "").await?;

    tokio::spawn(async move {
        while let Some(mut connection) = server.accept().await {
            let router_clone = Arc::clone(&shared_router);
            tokio::spawn(async move {
                while let Ok(request) = connection.receive_request().await {
                    let _ = router_clone.handle_request(request, &mut connection).await;
                }
                connection.wait_for_close().await;
            });
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = BxpClient::connect("127.0.0.1:4434", "localhost").await?;

    println!("🔵 [Client] Requesting video file...");
    client.send_request(1, BxpAction::Fetch, "bxp://assets/movie.mp4").await?;
    let res = client.receive_response().await?;
    
    if res.status == BxpStatus::Success {
        println!("🔵 [Client] Server accepted request. Receiving payload stream...");
        
        let (stream_req_id, mut network_stream) = client.read_data_stream().await?;
        assert_eq!(stream_req_id, 1); 

        let mut downloaded_data = Vec::new();
        let bytes_written = tokio::io::copy(&mut network_stream, &mut downloaded_data).await?;
        
        println!("🔵 [Client] Successfully streamed {} bytes: {}", 
            bytes_written, 
            String::from_utf8_lossy(&downloaded_data)
        );
    }

    client.close().await;
    Ok(())
}