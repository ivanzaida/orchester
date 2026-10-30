//! A shared project's configuration (`docs/architecture/server.md`, "Shared configuration", S5): its shapes and
//! errors round-trip as the server answers them, and each client call goes to its endpoint. The catalog of what the
//! web editor can pick (row 75) too.
//!
//! `cargo test -p orchester-api --features client --test resources resources_`
//! `cargo test -p orchester-api --features client --test resources catalog`

mod catalog;
#[cfg(feature = "client")]
mod catalog_client;
#[cfg(feature = "client")]
mod client;
mod errors;
mod fixtures;
mod shapes;
