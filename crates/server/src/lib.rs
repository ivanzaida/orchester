//! Accounts and transport for the self-hosted shared server.

mod accounts;
pub mod addresses;
pub mod clock;
pub mod config;
mod connection_limit;
mod db;
pub mod error;
mod http;
mod initial_read;
mod passwords;
mod rate_limit;
mod sharing;
mod tls;
mod transport;

pub use accounts::Server;
pub use db::backup_existing;
pub use transport::serve;
