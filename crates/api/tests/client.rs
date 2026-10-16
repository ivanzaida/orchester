//! The desktop's client against a real server on loopback: what `GET /server` tells, joining with a
//! code, sessions, sign-out and the admin user endpoints; plain HTTP, redirects, things that are not
//! an Orchester server, untrusted certificates and a pinned certificate; shared projects, their
//! members and roles, tasks under a revision, claims, the heartbeat and tags. `client_redaction` checks
//! that tokens and codes never show in `Debug` output or logs.
//!
//! `cargo test -p orchester-api --features client --test client <filter>`

mod support;

#[path = "client/accounts.rs"]
mod accounts;
#[path = "client/admin.rs"]
mod admin;
#[path = "client/sharing.rs"]
mod sharing;
#[path = "client/transport.rs"]
mod transport;
