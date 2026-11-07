//! What this computer fills in for the team's configuration (`docs/design/team-configuration.md`, "What a computer
//! fills in"): the connection (and account) a team profile runs on, the program path of a team MCP server, secret
//! values by name, and which team MCP servers are on here. The writes check that what they name is the team's, then
//! keep it in the stores of `orchester-store`.
//!
//! Turning a server on and setting its program path widen what agents may do, so they are a person's alone: the
//! window passes [`Answerer::Person`](orchester_core::run::Answerer::Person) only for a click it took from a person,
//! and automation, which passes [`Answerer::Automation`](orchester_core::run::Answerer::Automation), is refused
//! [`FillInError::PersonOnly`] with nothing changed. Turning off and clearing are always taken. Every write is counted
//! in the workspace's [`OutsideChanges`](crate::tasks::outside::OutsideChanges) as a change of the project's
//! connections, so the screens that show them read again; a program path, which is this computer's for the whole team
//! server, is counted for every project shared on that server.

mod connection;
mod error;
mod program;
mod reviewer;
mod secret;
mod switch;
mod team;

pub use self::error::FillInError;
