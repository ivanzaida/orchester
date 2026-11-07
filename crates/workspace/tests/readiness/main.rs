//! Whether each profile, MCP server, tool and pipeline of a project is ready on this computer, or what is missing, in
//! a team project and in a local one, and that the read follows the fill-ins and the cached versions.
//!
//! `cargo test -p orchester-workspace --test readiness readiness`

mod follows;
mod groups;
mod local;
mod pending;
mod pipelines;
mod servers;
mod support;
mod team;
mod tools;
