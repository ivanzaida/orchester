//! The self-hosted team server: accounts and devices, codes, invites, the Orchester Cloud link, and shared projects.

mod accounts;
pub mod addresses;
mod approvals;
pub mod clock;
mod cloud_link;
mod codes;
pub mod config;
mod connection_limit;
mod db;
mod devices;
pub mod error;
pub mod host;
mod http;
mod initial_read;
mod invites;
mod private_file;
mod rate_limit;
mod server_keys;
mod sharing;
mod tls;
mod transport;

pub use accounts::Server;
pub use db::backup_existing;
pub use transport::serve;
