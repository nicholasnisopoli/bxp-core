// examples/echo_server.rs
use anyhow::Result;
use bxp_node::{BxpServer, Action}; // Use your crate name here

#[tokio::main]
async fn main() -> Result<()> {
    let server = BxpServer::bind("127.0.0.1:4433").await?;
    println!("🟢 Echo Server listening on 127.0.0.1:4433...");

    while let Some(mut connection) = server.accept().await {
        tokio::spawn(async move {
            println!("   -> Client connected!");

            // 1. Read the Control Request
            let request = connection.receive_request().await.unwrap();
            println!("   -> Received Request ID: {} | Action: {:?}", request.req_id, request.action);

            if matches!(request.action, Action::Push) {
                // 2. Read the incoming payload stream
                let payload = connection.read_data_stream().await.unwrap();
                println!("   -> Received Payload: {}", String::from_utf8_lossy(&payload));

                // 3. Send Success Response
                connection.send_response(0x01).await.unwrap();

                // 4. Echo the payload back on a new stream
                connection.send_data_stream(&payload).await.unwrap();
                println!("   -> Echoed payload back to client.\n");
            }
            
            // 5. THE FIX: Wait for the client to hang up!
            connection.wait_for_close().await;
            println!("   -> Client disconnected gracefully.");
        });
    }
    Ok(())
}