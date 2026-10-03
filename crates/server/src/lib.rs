//! Accounts and transport for the self-hosted shared server.

mod accounts;
pub mod addresses;
pub mod clock;
pub mod config;
mod db;
pub mod error;
mod http;
mod passwords;
mod rate_limit;
mod tls;
mod transport;

pub use accounts::Server;
pub use transport::serve;
