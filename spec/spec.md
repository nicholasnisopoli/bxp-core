# BXP (Binary eXchange Protocol) Specification

**Version:** Draft v0.1
**Transport:** QUIC (UDP)
**ALPN:** `bxp/1`

---

## 1. Abstract

BXP is a high-performance, zero-copy RPC and bulk data transfer protocol built natively over QUIC. It addresses the CPU serialization overhead and Head-of-Line (HoL) blocking inherent in HTTP/2 and gRPC by cleanly separating control metadata from payload data.

Control metadata is strictly typed using Cap'n Proto and routed over a persistent bidirectional stream. Bulk payloads are transferred as raw, unformatted binary bytes over ephemeral unidirectional streams, bypassing system RAM limitations entirely.

---

## 2. Transport Layer Requirements

BXP mandates the use of **QUIC** (RFC 9000) and **TLS 1.3** (RFC 8446).

- **ALPN Token:** Connections MUST negotiate the ALPN token `bxp/1`.
- **Keep-Alive:** Endpoints SHOULD send keep-alive frames during idle periods (recommended interval: 10 seconds) to prevent stateful firewalls from dropping the UDP connection.
- **Security:** Unencrypted connections are strictly forbidden.

---

## 3. Protocol Architecture (The Split-Plane)

BXP multiplexes two distinct data planes over a single QUIC connection:

1. **The Control Plane (Stream 0):** A single, long-lived bidirectional stream dedicated solely to exchanging Cap'n Proto metadata (Requests and Responses).
2. **The Data Plane (Streams 1..N):** Ephemeral, unidirectional streams spawned dynamically to transfer raw binary payloads associated with a specific request.

---

## 4. The Control Plane (Stream 0)

Upon establishing the QUIC connection, the client MUST immediately open a bidirectional stream. This stream handles all routing and status communication.

### 4.1 Message Framing

Because QUIC streams do not preserve message boundaries, BXP implements a strict Length-Prefixed framing protocol on the Control Plane.

Every Request and Response sent over Stream 0 MUST follow this binary layout:

```
  0                   1                   2                   3
  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                 Message Length (32-bit, Little Endian)        |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                                                               |
 .               Cap'n Proto Serialized Message                  .
 .                                                               .
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Message Length:** A 4-byte unsigned integer (Little Endian) indicating the size of the subsequent Cap'n Proto message in bytes. Servers SHOULD reject lengths exceeding 1,048,576 bytes (1 MB) to prevent allocation attacks.
- **Payload:** The Cap'n Proto message (either a Request or Response).

### 4.2 Cap'n Proto Schema

The Control Plane strictly adheres to the following Cap'n Proto schema:

```capnp
@0xabcdef1234567890;

enum Action {
  fetch @0;
  push  @1;
  ping  @2;
}

enum StatusCode {
  success       @0;   
  badRequest    @1;  
  unauthorized  @2;  
  notFound      @3;   
  internalError @4; 
}

struct Request {
  requestId   @0 :UInt32;
  action      @1 :Action;
  resourceUri @2 :Text;
}

struct Response {
  requestId  @0 :UInt32;
  statusCode @1 :StatusCode;
}
```

---

## 5. The Data Plane (Streams 1..N)

When a Request requires the transfer of a bulk payload (e.g., a file upload or download), the sending party MUST open a new Unidirectional QUIC Stream.

### 5.1 Stream Association (The Preamble)

Because QUIC stream creation is asynchronous and network routes vary, unidirectional streams may arrive out-of-order. To associate an incoming Data Stream with its parent Request, the first 4 bytes of every Data Stream MUST contain the `requestId`.

### 5.2 Data Stream Framing

Data streams use the following layout:

```
  0                   1                   2                   3
  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                  Request ID (32-bit, Little Endian)           |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                                                               |
 .                  Raw Unformatted Binary Bytes                 .
 .                 (Streamed until FIN/EOF received)             .
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Request ID:** A 4-byte unsigned integer (Little Endian) matching the `requestId` from the Cap'n Proto control message.
- **Payload:** Raw bytes. The stream MUST NOT be buffered entirely into memory. Receivers SHOULD pipe this stream directly to disk or a processing pipeline. The stream terminates when the QUIC FIN bit is received.

---

## 6. Connection Lifecycle & Examples

### Scenario A: A Simple Ping

1. Client writes to Stream 0: `[Length: 32] [Request: ID=1, Action=Ping, URI="bxp://health"]`
2. Server reads Stream 0, processes the route.
3. Server writes to Stream 0: `[Length: 16] [Response: ID=1, Status=Success]`

### Scenario B: Fetching a Large File

1. Client writes to Stream 0: `[Length: 48] [Request: ID=2, Action=Fetch, URI="bxp://data.bin"]`
2. Server reads Stream 0, locates the file.
3. Server writes to Stream 0: `[Length: 16] [Response: ID=2, Status=Success]`
4. Server opens a NEW Unidirectional Stream (e.g., Stream 3).
5. Server writes to Stream 3: `[ID=2 (4 bytes)] [50GB of raw file bytes...]`
6. Server gracefully closes Stream 3 (sends FIN).
7. Client receives Stream 3, reads the 4-byte preamble (`2`), realizes this is the payload for Request 2, and streams the remaining bytes to disk.

---

## 7. Error Handling & Disconnects

- **Invalid Control Messages:** If an endpoint receives a malformed Cap'n Proto message or a length header exceeding 1 MB, it MUST immediately terminate the QUIC connection with an application error code of `1` (`Control Stream Failed`).
- **Missing Routes:** If a server receives a Request for a `resourceUri` it does not recognize, it MUST reply on Stream 0 with a Response containing `StatusCode::notFound`. It MUST NOT open a Data Stream.
- **Graceful Shutdown:** To disconnect, endpoints SHOULD send a standard QUIC connection close frame.