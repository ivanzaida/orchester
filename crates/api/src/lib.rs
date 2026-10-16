//! The shared server's and Orchester Cloud's versioned JSON contract, with the `client` feature the HTTP client the
//! desktop uses, and with the `identity` feature the cloud's identity tokens and key sets and its two-factor codes.

mod accounts;
pub mod certificate;
#[cfg(feature = "client")]
pub mod client;
pub mod cloud;
mod credential;
mod error;
mod events;
#[cfg(feature = "identity")]
pub mod identity;
mod invites;
mod joins;
mod projects;
mod server_admin;
pub mod statements;
mod tasks;
#[cfg(feature = "identity")]
pub mod totp;
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
