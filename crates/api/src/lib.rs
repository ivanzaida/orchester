//! The shared server's versioned JSON contract, and with the `client` feature the HTTP client the
//! desktop uses.

mod accounts;
pub mod certificate;
#[cfg(feature = "client")]
pub mod client;
mod credential;
mod error;
mod events;
mod invites;
mod joins;
mod projects;
mod server_admin;
mod tasks;
pub mod wire;

pub use accounts::*;
pub use credential::Credential;
pub use error::{ApiError, ErrorCode, SharingError, SharingErrorDetails};
pub use events::*;
pub use invites::*;
pub use joins::*;
pub use projects::*;
pub use server_admin::*;
pub use tasks::*;
