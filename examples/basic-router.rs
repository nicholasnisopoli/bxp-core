// examples/basic_router.rs

use anyhow::Result;
use bxp_core::{
    BxpAction, BxpClient, BxpRequest, BxpRouter, BxpServer, BxpServerConnection, BxpStatus,
};
use std::sync::Arc;
use tokio::time::Duration;

// --- Clean, isolated handler function ---
async fn handle_ping(req: BxpRequest, conn: &mut BxpServerConnection) -> Result<()> {
    println!("🟢 [Server] Received Ping (ID: {}) on {}", req.req_id, req.uri);
    conn.send_response(req.req_id, BxpStatus::Success).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting BXP Basic Router Demo...\n");

    let router = BxpRouter::new().route(BxpAction::Ping, "bxp://health", handle_ping);
    let shared_router = Arc::new(router);

    let server = BxpServer::bind("127.0.0.1:4433", "", "").await?;
    println!("📡 Server listening on 127.0.0.1:4433...");

    tokio::spawn(async move {
        while let Some(mut connection) = server.accept().await {
            let router_clone = Arc::clone(&shared_router);
            tokio::spawn(async move {
                while let Ok(request) = connection.receive_request().await {
                    if let Err(e) = router_clone.handle_request(request, &mut connection).await {
                        eprintln!("🔴 [Server] Handler Error: {:?}", e);
                    }
                }
                connection.wait_for_close().await;
            });
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    println!("🔵 [Client] Connecting to BXP Server...");
    let mut client = BxpClient::connect("127.0.0.1:4433", "localhost").await?;

    println!("\n🔵 [Client] Testing Ping Endpoint...");
    client.send_request(1, BxpAction::Ping, "bxp://health").await?;
    
    let res = client.receive_response().await?;
    if res.status == BxpStatus::Success {
        println!("🔵 [Client] Ping Successful! Server is healthy.");
    }

    println!("\n🏁 Disconnecting...");
    client.close().await;
    Ok(())
}