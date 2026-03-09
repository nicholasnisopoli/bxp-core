// src/protocol.rs

/// This module defines the core protocol types and conversions for BXP, including the `BxpAction` and `BxpStatus` enums, as well as the `BxpRequest` and `BxpResponse` structs. These types are used throughout the client, server, and router modules to represent requests, responses, and actions in a clean and type-safe manner. The module also includes conversions to and from the Cap'n Proto generated types for seamless serialization and deserialization over the network.
/// The `BxpAction` enum represents the different types of actions that can be performed (e.g., Fetch, Push, Ping), while the `BxpStatus` enum represents the possible status codes for responses (e.g., Success, BadRequest). The `BxpRequest` struct encapsulates the details of a request, including its ID, action, and URI, while the `BxpResponse` struct encapsulates the response details, including the request ID and status. These types are designed to be ergonomic and easy to use in the context of handling BXP requests and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] 
pub enum BxpAction {
    Fetch = 0,
    Push = 1,
    Ping = 2,
}

impl From<BxpAction> for crate::bxp_capnp::Action {
    fn from(action: BxpAction) -> Self {
        match action {
            BxpAction::Fetch => crate::bxp_capnp::Action::Fetch,
            BxpAction::Push => crate::bxp_capnp::Action::Push,
            BxpAction::Ping => crate::bxp_capnp::Action::Ping,
        }
    }
}

impl TryFrom<crate::bxp_capnp::Action> for BxpAction {
    type Error = anyhow::Error;

    fn try_from(value: crate::bxp_capnp::Action) -> Result<Self, Self::Error> {
        match value {
            crate::bxp_capnp::Action::Fetch => Ok(BxpAction::Fetch),
            crate::bxp_capnp::Action::Push => Ok(BxpAction::Push),
            crate::bxp_capnp::Action::Ping => Ok(BxpAction::Ping),
        }
    }
}

/// The BxpRequest struct represents a request sent by the client to the server, containing a unique request ID, the action to be performed, and the resource URI. The BxpResponse struct represents the response from the server, containing the request ID for correlation and the status of the request. These structs are designed to be simple and easy to use when sending requests and receiving responses in the BXP protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BxpStatus {
    Success = 0,
    BadRequest = 1,
    Unauthorized = 2,
    NotFound = 3,
    InternalError = 4,
}

impl From<BxpStatus> for crate::bxp_capnp::StatusCode {
    fn from(status: BxpStatus) -> Self {
        match status {
            BxpStatus::Success => crate::bxp_capnp::StatusCode::Success,
            BxpStatus::BadRequest => crate::bxp_capnp::StatusCode::BadRequest,
            BxpStatus::Unauthorized => crate::bxp_capnp::StatusCode::Unauthorized,
            BxpStatus::NotFound => crate::bxp_capnp::StatusCode::NotFound,
            BxpStatus::InternalError => crate::bxp_capnp::StatusCode::InternalError,
        }
    }
}

impl TryFrom<crate::bxp_capnp::StatusCode> for BxpStatus {
    type Error = anyhow::Error;

    fn try_from(value: crate::bxp_capnp::StatusCode) -> Result<Self, Self::Error> {
        match value {
            crate::bxp_capnp::StatusCode::Success => Ok(BxpStatus::Success),
            crate::bxp_capnp::StatusCode::BadRequest => Ok(BxpStatus::BadRequest),
            crate::bxp_capnp::StatusCode::Unauthorized => Ok(BxpStatus::Unauthorized),
            crate::bxp_capnp::StatusCode::NotFound => Ok(BxpStatus::NotFound),
            crate::bxp_capnp::StatusCode::InternalError => Ok(BxpStatus::InternalError),
        }
    }
}

// --- CORE STRUCTS ---
/// The BxpRequest struct represents a request sent by the client to the server, containing a unique request ID, the action to be performed, and the resource URI. The BxpResponse struct represents the response from the server, containing the request ID for correlation and the status of the request. These structs are designed to be simple and easy to use when sending requests and receiving responses in the BXP protocol.
#[derive(Debug, Clone)]
pub struct BxpRequest {
    pub req_id: u32, /// An opaque identifier chosen by the client to correlate requests and responses. The server does not validate this ID, but simply echoes it back in the response for correlation purposes.
    pub action: BxpAction, /// The action to be performed (e.g., Fetch, Push, Ping). This is a strongly typed enum that maps directly to the Cap'n Proto Action type for seamless serialization.
    pub uri: String // The resource URI associated with the request (e.g., "bxp://example.com/resource"). This is a simple string that can be parsed and handled by the server's routing logic.
}
/// The BxpResponse struct represents the response from the server, containing the request ID for correlation and the status of the request. These structs are designed to be simple and easy to use when sending requests and receiving responses in the BXP protocol.
#[derive(Debug, Clone)]
pub struct BxpResponse {
    pub req_id: u32, /// The request ID from the original request, echoed back by the server for correlation. The client can choose to ignore this field or use it to match responses to requests, but the server does not enforce any validation on it.
    pub status: BxpStatus, // The status of the request (e.g., Success, BadRequest). This is a strongly typed enum that maps directly to the Cap'n Proto StatusCode type for seamless serialization.
}