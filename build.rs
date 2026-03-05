use capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/bxp.capnp")
        .run()
        .expect("Failed to compile Cap'n Proto schema");

    // 2. Compile Protobuf schema (gRPC Benchmark)
    tonic_build::compile_protos("schema/benchmark.proto")
        .expect("Failed to compile Protobuf schema");
}