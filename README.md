# BXP (Binary eXchange Protocol) 

![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)
![Rust Version](https://img.shields.io/badge/rust-1.93%2B-orange.svg)
![Status: Experimental](https://img.shields.io/badge/status-experimental-red.svg)

**BXP** is a high-performance, binary-first data transfer protocol built on **QUIC** and **Cap'n Proto**. 

Designed as a modern, high-throughput alternative to legacy HTTP/REST for microservices and distributed systems, BXP eliminates text-parsing CPU overhead and head-of-line blocking by using zero-copy binary serialization and native stream multiplexing.



## Why BXP?

HTTP (even H2 and H3) carries legacy baggage: string-based headers, verbs designed for document retrieval, and heavy CPU parsing overhead. BXP starts from a clean slate:

* **Zero-Copy Parsing:** Control messages are strictly typed using Cap'n Proto. The bytes received from the network socket are read directly as memory structs without a decoding or allocation phase.
* **Built on QUIC:** Powered by `quinn`, BXP inherits mandatory TLS 1.3 encryption, 0-RTT handshakes, and true stream multiplexing.
* **Separation of Concerns:** 
    * **Stream 0 (Control Plane):** A persistent, bidirectional stream exclusively for authentication, metadata, and routing.
    * **Streams 1-N (Data Plane):** Ephemeral, unidirectional streams spun up instantly to blast raw binary payloads without blocking administrative traffic.

## Installation

Add `bxp-core` to your Rust project:

```bash
cargo add bxp-core
```

## Architecture
<img width="1820" height="1104" alt="image" src="https://github.com/user-attachments/assets/0a25d57d-8126-4a65-999d-70a272163b5c" />

## Performance Benchmarks

BXP was benchmarked against `tonic` (the premier Rust gRPC framework) over 1,000,000 sequential requests on a single multiplexed connection. 

Because BXP utilizes **zero-copy Cap'n Proto serialization** and avoids **HTTP/2 header parsing**, it consistently delivers higher throughput and vastly lower median latency than gRPC.

| Metric (1M Requests) | gRPC (HTTP/2 + Protobuf) | BXP (QUIC + Cap'n Proto) | Improvement |
| :--- | :--- | :--- | :--- |
| **Total Time** | 94.84s | **75.47s** | **20% Faster** |
| **Throughput** | 10,543 req/sec | **13,249 req/sec** | **+25% Throughput** |
| **p50 Latency** | 89.30 µs | **64.10 µs** | **28% Lower Latency** |
| **p90 Latency** | 119.20 µs | **110.80 µs** | **7% Lower Latency** |

*(Note: Benchmarks run in release mode on standard consumer hardware. p99 latencies converge near 170µs as the OS scheduler and async runtime become the primary hardware bottleneck).*
