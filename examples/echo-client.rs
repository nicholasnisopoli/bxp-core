use anyhow::Result;
use bxp_node::{BxpClient, Action}; 

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔵 Connecting to Echo Server...");
    let mut client = BxpClient::connect("127.0.0.1:4433").await?;

    let payload = b"Hello BXP! This is a high-speed zero-copy message.";

    // 1. Send Request metadata on Stream 0
    client.send_request(101, Action::Push, "bxp://echo").await?;
    
    // 2. Blast the data on a Unidirectional Stream
    client.send_data_stream(payload).await?;
    println!("   -> Sent payload to server.");

    // 3. Read the Control Response
    let response = client.receive_response().await?;
    println!("   -> Server Response Code: 0x{:02X}", response.status_code);

    // 4. Read the echoed data back
    let echoed_data = client.read_data_stream().await?;
    println!("   -> Echoed Data: {}", String::from_utf8_lossy(&echoed_data));

    // 5. Hang up
    client.close().await;
    println!("🔵 Connection closed gracefully.");
    Ok(())
}