//! Types only: the shared server's versioned JSON contract.

mod accounts;
mod credential;
mod error;

pub use accounts::*;
pub use credential::Credential;
pub use error::{ApiError, ErrorCode};
