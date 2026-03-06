// src/protocol.rs

// --- THE NEW BXP ACTION ENUM ---
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] // Look! Hash and Eq are here!
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

// --- EXISTING BXP STATUS ENUM ---
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

#[derive(Debug, Clone)]
pub struct BxpRequest {
    pub req_id: u32,
    pub action: BxpAction, // Use our new clean enum here!
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct BxpResponse {
    pub req_id: u32,
    pub status: BxpStatus,
}