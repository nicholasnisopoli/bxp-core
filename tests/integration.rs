// tests/integration_test.rs
use anyhow::Result;
use bxp_core::{
    BxpAction, BxpClient, BxpRequest, BxpRouter, BxpServer, BxpServerConnection, BxpStatus,
};
use std::sync::Arc;

async fn spawn_test_server(router: BxpRouter) -> Result<String> {
    let shared_router = Arc::new(router);
    let server = BxpServer::bind("127.0.0.1:0", "", "").await?;
    let addr = server.local_addr()?.to_string();

    tokio::spawn(async move {
        if let Some(mut connection) = server.accept().await {
            while let Ok(request) = connection.receive_request().await {
                let _ = shared_router.handle_request(request, &mut connection).await;
            }
            connection.wait_for_close().await;
        }
    });

    Ok(addr)
}

// --- 1. Define clean, normal async functions for our handlers! ---

async fn handle_ping(req: BxpRequest, conn: &mut BxpServerConnection) -> Result<()> {
    conn.send_response(req.req_id, BxpStatus::Success).await?;
    Ok(())
}

async fn handle_file_transfer(req: BxpRequest, conn: &mut BxpServerConnection) -> Result<()> {
    conn.send_response(req.req_id, BxpStatus::Success).await?;
    let payload = std::io::Cursor::new(b"Hello, BXP Zero-Copy World!");
    conn.send_data_stream(req.req_id, payload).await?;
    Ok(())
}

// --- 2. The Tests ---

#[tokio::test]
async fn test_router_ping_and_404() -> Result<()> {
    // Look how clean the routing is now! 
    let router = BxpRouter::new().route(BxpAction::Ping, "bxp://ping", handle_ping);
    let addr = spawn_test_server(router).await?;
    let mut client = BxpClient::connect(&addr, "localhost").await?;

    // Valid Ping
    client.send_request(100, BxpAction::Ping, "bxp://ping").await?;
    let response = client.receive_response().await?;
    assert_eq!(response.status, BxpStatus::Success);

    // 404 Not Found
    client.send_request(101, BxpAction::Fetch, "bxp://does-not-exist").await?;
    let response2 = client.receive_response().await?;
    assert_eq!(response2.status, BxpStatus::NotFound);

    client.close().await;
    Ok(())
}

#[tokio::test]
async fn test_data_stream_transfer() -> Result<()> {
    // Just pass the function pointer!
    let router = BxpRouter::new().route(BxpAction::Fetch, "bxp://file", handle_file_transfer);
    let addr = spawn_test_server(router).await?;
    let mut client = BxpClient::connect(&addr, "localhost").await?;

    client.send_request(55, BxpAction::Fetch, "bxp://file").await?;
    let response = client.receive_response().await?;
    assert_eq!(response.status, BxpStatus::Success);

    let (stream_req_id, mut network_stream) = client.read_data_stream().await?;
    assert_eq!(stream_req_id, 55);

    let mut downloaded_data = Vec::new();
    tokio::io::copy(&mut network_stream, &mut downloaded_data).await?;
    assert_eq!(downloaded_data, b"Hello, BXP Zero-Copy World!");

    client.close().await;
    Ok(())
}