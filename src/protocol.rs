// src/protocol.rs
// Now we just import from the root crate
pub use crate::bxp_capnp::Action;

/// A clean, owned Rust struct representing a BXP Request
#[derive(Debug, Clone)]
pub struct BxpRequest {
    pub req_id: u32,
    pub action: Action,
    pub uri: String,
}

/// A clean, owned Rust struct representing a BXP Response
#[derive(Debug, Clone)]
pub struct BxpResponse {
    pub req_id: u32,
    pub status_code: u16,
}