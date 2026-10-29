//! A shared project's configuration (`docs/architecture/server.md`, "Shared configuration", S5): its shapes and
//! errors round-trip as the server answers them, and each client call goes to its endpoint.
//!
//! `cargo test -p orchester-api --features client --test resources resources_`

#[cfg(feature = "client")]
mod client;
mod errors;
mod fixtures;
mod shapes;
