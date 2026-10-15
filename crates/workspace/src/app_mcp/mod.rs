//! The tools of Orchester's own MCP server (`docs/stable-build.md`, "Orchester's MCP server"):
//! automation, such as the coordinator agent that develops Orchester through Orchester, controls
//! projects, tasks, runs, approvals and pipelines through data-level tools instead of clicking the
//! window. Each tool goes through the services the screens use ([`Services`]), so it follows the same
//! rules, writes the same records and raises the same events. The server around them (its transport,
//! guard and discovery file) is each app's.

mod error;
mod services;
mod tools;

pub(crate) use self::services::blocking;
pub use self::{
  error::{Missing, ToolError},
  services::Services,
  tools::{Proposal, Translate, call, find_run, listed, propose_run, run_proposed, setup},
};
