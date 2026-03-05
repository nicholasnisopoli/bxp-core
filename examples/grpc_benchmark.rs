use std::time::{Duration, Instant};
use tonic::{transport::Server, Request, Response, Status};

// Import the auto-generated gRPC Rust code
pub mod benchmark {
    tonic::include_proto!("benchmark");
}

use benchmark::benchmark_service_server::{BenchmarkService, BenchmarkServiceServer};
use benchmark::benchmark_service_client::BenchmarkServiceClient;
use benchmark::{BenchmarkRequest, BenchmarkResponse, Action};

const NUM_REQUESTS: u32 = 1_000_000;

// Define our gRPC Server
#[derive(Default)]
pub struct MyBenchmarkServer {}

#[tonic::async_trait]
impl BenchmarkService for MyBenchmarkServer {
    async fn fetch(&self, request: Request<BenchmarkRequest>) -> Result<Response<BenchmarkResponse>, Status> {
        let req = request.into_inner();
        
        // Return an instant response, exactly like the BXP server does
        Ok(Response::new(BenchmarkResponse {
            req_id: req.req_id,
            status_code: 1, // 0x01 Success
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🐢 Starting gRPC Benchmark: {} Requests", NUM_REQUESTS);

    let addr = "127.0.0.1:4436".parse()?;

    // 1. Start the gRPC Server in the background
    let server = MyBenchmarkServer::default();
    tokio::spawn(async move {
        Server::builder()
            .add_service(BenchmarkServiceServer::new(server))
            .serve(addr)
            .await
            .unwrap();
    });

    // Give the server a tiny fraction of a second to boot
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 2. Connect the gRPC Client
    let mut client = BenchmarkServiceClient::connect("http://127.0.0.1:4436").await?;
    
    let mut latencies = Vec::with_capacity(NUM_REQUESTS as usize);
    let benchmark_start = Instant::now();

    // 3. Blast the Requests
    for i in 0..NUM_REQUESTS {
        let req_start = Instant::now();
        
        // Construct the Protobuf Request (requires memory allocation)
        let request = tonic::Request::new(BenchmarkRequest {
            req_id: i,
            action: Action::Fetch.into(),
            resource_uri: "grpc://benchmark/resource".into(),
        });
        
        // Await the response
        let _response = client.fetch(request).await?;
        
        latencies.push(req_start.elapsed());
    }

    let total_time = benchmark_start.elapsed();

    // 4. Calculate Statistics
    latencies.sort();
    let p50 = latencies[(NUM_REQUESTS as f64 * 0.50) as usize];
    let p90 = latencies[(NUM_REQUESTS as f64 * 0.90) as usize];
    let p99 = latencies[(NUM_REQUESTS as f64 * 0.99) as usize];
    let throughput = (NUM_REQUESTS as f64 / total_time.as_secs_f64()) as u64;

    println!("\n📊 --- gRPC Benchmark Results ---");
    println!("Total Requests : {}", NUM_REQUESTS);
    println!("Total Time     : {:.2?}", total_time);
    println!("Throughput     : {} req/sec", throughput);
    println!("--------------------------------");
    println!("Latency p50    : {:.2?}", p50);
    println!("Latency p90    : {:.2?}", p90);
    println!("Latency p99    : {:.2?}", p99);
    println!("--------------------------------\n");

    Ok(())
}