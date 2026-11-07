//! What this computer fills in for the team's configuration: the connection a team profile runs on, the program of a
//! team MCP server, secret values by name, and which team MCP servers are on here. Each write checks that it names
//! the team's, turning on and setting a program take a person, and no secret value reaches an error or `Debug`.
//!
//! `cargo test -p orchester-workspace --test team_fill_ins team_fill_ins`

mod connection;
mod program;
mod secret;
mod sheet;
mod support;
mod switch;
mod unresolved;
