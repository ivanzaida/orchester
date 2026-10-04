//! The desktop's client against a real server on loopback: what `GET /server` tells, sign-in with
//! the forced new password, sessions, sign-out and the admin user endpoints; plain HTTP, redirects,
//! things that are not an Orchester server and untrusted certificates. `client_redaction` checks
//! that the token and passwords never show in `Debug` output or logs.
//!
//! `cargo test -p orchester-api --features client --test client <filter>`

mod support;

#[path = "client/accounts.rs"]
mod accounts;
#[path = "client/admin.rs"]
mod admin;
#[path = "client/transport.rs"]
mod transport;
