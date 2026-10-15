//! Orchester Cloud: one service, run by the person for everyone, that says who people are. Sign-up and sign-in by email
//! and password, devices, and identity tokens team servers verify locally (`docs/architecture/server.md`, "Orchester
//! Cloud"). It never holds project data.

mod accounts;
pub mod addresses;
pub mod clock;
pub mod config;
mod db;
pub mod error;
mod http;
mod keys;
pub mod limits;
pub mod mail;
pub mod passwords;
mod sweep;
mod tokens;

/// The service's log filter: the cloud's own structured records only. Libraries' debug logs can hold request data
/// and SQL parameters (`tokio-postgres` logs every statement's parameters at debug), so they are never enabled.
pub const LOG_FILTER: &str = "orchester_cloud=info";

pub use accounts::Cloud;
pub use db::LATEST_SCHEMA;
pub use http::serve;
