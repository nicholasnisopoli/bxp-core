use anyhow::Result;
use bxp_node::{BxpServer, Action};

#[tokio::main]
async fn main() -> Result<()> {
    let server = BxpServer::bind("127.0.0.1:4434").await?;
    println!("🟢 File Server listening on 127.0.0.1:4434...");

    while let Some(mut connection) = server.accept().await {
        tokio::spawn(async move {
            println!("   -> Client connected!");

            let request = connection.receive_request().await.unwrap();
            
            if matches!(request.action, Action::Fetch) {
                println!("   -> Client requested: {}", request.uri);
                
                // Simulate loading a file from disk
                let mut dummy_file_data = Vec::new();
                for i in 0..1000 {
                    dummy_file_data.extend_from_slice(format!("Line {} of the requested file.\n", i).as_bytes());
                }

                // Send success response
                connection.send_response(0x01).await.unwrap();

                // Blast the raw file data
                connection.send_data_stream(&dummy_file_data).await.unwrap();
                println!("   -> File streamed successfully ({} bytes).\n", dummy_file_data.len());
            } else {
                // Send error response if they didn't Fetch
                connection.send_response(0xE001).await.unwrap(); 
            }
            connection.wait_for_close().await;
            println!("   -> Client disconnected gracefully.");
        });
    }
    Ok(())
}