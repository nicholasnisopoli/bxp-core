// examples/benchmark.rs

use anyhow::Result;
use bxp_core::{
    BxpAction, BxpClient, BxpRequest, BxpRouter, BxpServer, BxpServerConnection, BxpStatus,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

const NUM_REQUESTS: u32 = 10_000;

// --- Clean, isolated handler function ---
async fn handle_benchmark_ping(req: BxpRequest, conn: &mut BxpServerConnection) -> Result<()> {
    conn.send_response(req.req_id, BxpStatus::Success).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting BXP Router Performance Benchmark: {} Requests", NUM_REQUESTS);

    // Look how clean this is now! Just pass the function pointer.
    let router = BxpRouter::new().route(BxpAction::Ping, "bxp://benchmark", handle_benchmark_ping);

    let shared_router = Arc::new(router);
    let server = BxpServer::bind("127.0.0.1:4435", "", "").await?;

    tokio::spawn(async move {
        while let Some(mut connection) = server.accept().await {
            let router_clone = Arc::clone(&shared_router);
            tokio::spawn(async move {
                while let Ok(request) = connection.receive_request().await {
                    let _ = router_clone.handle_request(request, &mut connection).await;
                }
            });
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = BxpClient::connect("127.0.0.1:4435", "localhost").await?;
    let mut latencies = Vec::with_capacity(NUM_REQUESTS as usize);
    let benchmark_start = Instant::now();

    for i in 0..NUM_REQUESTS {
        let req_start = Instant::now();
        
        client.send_request(i, BxpAction::Ping, "bxp://benchmark").await?;
        let _response = client.receive_response().await?;
        
        latencies.push(req_start.elapsed());
    }

    let total_time = benchmark_start.elapsed();

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