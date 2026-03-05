// examples/benchmark.rs
use anyhow::Result;
// 👇 Fixed the crate name right here!
use bxp_node::{BxpClient, BxpServer, Action}; 
use std::time::{Duration, Instant};

const NUM_REQUESTS: u32 = 1_000_000;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting BXP Benchmark: {} Requests", NUM_REQUESTS);

    // 1. Start the Server in the background
    let server = BxpServer::bind("127.0.0.1:4435").await?;
    tokio::spawn(async move {
        while let Some(mut connection) = server.accept().await {
            tokio::spawn(async move {
                // Loop continuously to handle multiple requests on the same connection
                while let Ok(_request) = connection.receive_request().await {
                    // Instantly acknowledge the request
                    if connection.send_response(0x01).await.is_err() {
                        break;
                    }
                }
            });
        }
    });

    // Give the server a tiny fraction of a second to boot
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 2. Connect the Client
    let mut client = BxpClient::connect("127.0.0.1:4435").await?;
    
    let mut latencies = Vec::with_capacity(NUM_REQUESTS as usize);
    let benchmark_start = Instant::now();

    // 3. Blast the Requests
    for i in 0..NUM_REQUESTS {
        let req_start = Instant::now();
        
        // Send a strictly typed binary request (Zero-copy layout)
        client.send_request(i, Action::Fetch, "bxp://benchmark/resource").await?;
        
        // Wait for the binary response
        let _response = client.receive_response().await?;
        
        latencies.push(req_start.elapsed());
    }

    let total_time = benchmark_start.elapsed();

    // 4. Calculate Statistics
    latencies.sort();
    let p50 = latencies[(NUM_REQUESTS as f64 * 0.50) as usize];
    let p90 = latencies[(NUM_REQUESTS as f64 * 0.90) as usize];
    let p99 = latencies[(NUM_REQUESTS as f64 * 0.99) as usize];
    let throughput = (NUM_REQUESTS as f64 / total_time.as_secs_f64()) as u64;

    println!("\n📊 --- BXP Benchmark Results ---");
    println!("Total Requests : {}", NUM_REQUESTS);
    println!("Total Time     : {:.2?}", total_time);
    println!("Throughput     : {} req/sec", throughput);
    println!("--------------------------------");
    println!("Latency p50    : {:.2?}", p50);
    println!("Latency p90    : {:.2?}", p90);
    println!("Latency p99    : {:.2?}", p99);
    println!("--------------------------------\n");

    client.close().await;
    Ok(())
}