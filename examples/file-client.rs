use anyhow::Result;
use bxp_node::{BxpClient, Action};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔵 Connecting to File Server...");
    let mut client = BxpClient::connect("127.0.0.1:4434").await?;

    let target_file = "bxp://server/assets/dataset.csv";
    println!("   -> Requesting file: {}", target_file);

    // 1. Send Fetch Request
    client.send_request(202, Action::Fetch, target_file).await?;

    // 2. Await Response
    let response = client.receive_response().await?;
    if response.status_code == 0x01 {
        println!("   -> Server accepted request. Receiving data stream...");

        // 3. Read the file stream
        let file_data = client.read_data_stream().await?;
        println!("   -> Successfully downloaded {} bytes!", file_data.len());
        
        // Print the first 100 characters to verify
        let preview = String::from_utf8_lossy(&file_data[0..100.min(file_data.len())]);
        println!("   -> Preview: {}...", preview.trim());
    } else {
        println!("   -> Server rejected request with code: 0x{:02X}", response.status_code);
    }

    client.close().await;
    Ok(())
}