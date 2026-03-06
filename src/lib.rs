// src/lib.rs
// 1. Move the Cap'n Proto module here to the crate root!
pub mod bxp_capnp {
    include!(concat!(env!("OUT_DIR"), "/bxp_capnp.rs"));
}

pub mod protocol;
pub mod server;
pub mod client;
pub mod router;

pub use protocol::{BxpAction, BxpRequest, BxpResponse, BxpStatus};
pub use server::{BxpServer, BxpServerConnection};
pub use client::{BxpClient, BxpClientConnection};
pub use router::{BxpRouter, BxpHandler};