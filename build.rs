use capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/bxp.capnp")
        .run()
        .expect("Failed to compile Cap'n Proto schema");
}