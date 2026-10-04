//! The shared server's versioned JSON contract, and with the `client` feature the HTTP client the
//! desktop uses.

mod accounts;
#[cfg(feature = "client")]
pub mod client;
mod credential;
mod error;
mod events;
mod projects;
mod tasks;
pub mod wire;

pub use accounts::*;
pub use credential::Credential;
pub use error::{ApiError, ErrorCode, SharingError, SharingErrorDetails};
pub use events::*;
pub use projects::*;
pub use tasks::*;
