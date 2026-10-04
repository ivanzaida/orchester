//! The shared server's versioned JSON contract, and with the `client` feature the HTTP client the
//! desktop uses.

mod accounts;
#[cfg(feature = "client")]
pub mod client;
mod credential;
mod error;

pub use accounts::*;
pub use credential::Credential;
pub use error::{ApiError, ErrorCode};
