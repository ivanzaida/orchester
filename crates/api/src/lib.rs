//! The shared server's and Orchester Cloud's versioned JSON contract, with the `client` feature the HTTP client the
//! desktop uses, and with the `identity` feature the cloud's identity tokens and key sets.

mod accounts;
#[cfg(feature = "client")]
pub mod client;
pub mod cloud;
mod credential;
mod error;
mod events;
#[cfg(feature = "identity")]
pub mod identity;
mod projects;
mod tasks;
pub mod wire;

pub use accounts::*;
pub use credential::Credential;
pub use error::{ApiError, ErrorCode, SharingError, SharingErrorDetails};
pub use events::*;
pub use projects::*;
pub use tasks::*;
