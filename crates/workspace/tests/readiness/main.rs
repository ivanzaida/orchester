//! Whether each profile, MCP server, tool and pipeline of a project is ready on this computer, or what is missing, in
//! a team project and in a local one, and that the read follows the fill-ins and the cached versions.
//!
//! `cargo test -p orchester-workspace --test readiness readiness`

mod archived;
mod follows;
mod groups;
mod hub;
mod local;
mod local_archive;
mod local_connection;
mod local_edit;
mod local_hub;
mod pending;
mod pipeline_hub;
mod pipelines;
mod reviewers;
mod servers;
mod start;
mod support;
mod team;
mod tools;
