//! Orchester's own MCP server, started by a headless window on loopback: its discovery file and
//! bearer token, and each tool's happy path and main refusal, through the services the screens use.
//! Run with `cargo test -p orchester-desktop --test form_keyboard app_mcp`.

mod answered;
pub(crate) mod approvals;
mod continue_run;
mod limits;
mod messages;
mod pipelines;
mod plans;
mod questions;
mod quiet;
mod quiet_screens;
mod return_to_draft;
mod runs;
mod server;
mod tasks;
mod worktree;

use std::{fs, path::PathBuf};

use serde_json::Value;

use crate::{harness::Desktop, mcp_agent::McpAgent};

/// What this process's discovery file says.
pub(crate) struct Discovered {
  pub(crate) path: PathBuf,
  pub(crate) port: u16,
  pub(crate) token: String,
  pub(crate) contents: Value,
}

/// The discovery file the window's server wrote.
pub(crate) fn discovered(desktop: &Desktop) -> Discovered {
  let path = desktop.mcp_folder().join(format!("{}.json", std::process::id()));
  let text = fs::read_to_string(&path).expect("the server wrote its discovery file");
  let contents: Value = serde_json::from_str(&text).expect("the discovery file is JSON");
  let port = contents["port"]
    .as_u64()
    .and_then(|port| u16::try_from(port).ok())
    .expect("a port");
  let token = contents["token"].as_str().expect("a token").to_owned();
  Discovered {
    path,
    port,
    token,
    contents,
  }
}

/// A client of the window's server, with the token of its discovery file.
pub(crate) fn agent(desktop: &Desktop) -> McpAgent {
  let found = discovered(desktop);
  McpAgent::connect(found.port, found.token)
}

/// The result of tool `name`, which must not be an error.
pub(crate) fn ok(agent: &mut McpAgent, name: &str, arguments: &Value) -> Value {
  let result = agent.call(name, arguments);
  assert_ne!(result["isError"], true, "{name} failed: {result}");
  result["structuredContent"].clone()
}

/// The error of tool `name`, which must refuse: its `code`, `message` and `reasons`.
pub(crate) fn refused(agent: &mut McpAgent, name: &str, arguments: &Value) -> Value {
  let result = agent.call(name, arguments);
  assert_eq!(result["isError"], true, "{name} did not refuse: {result}");
  result["structuredContent"]["error"].clone()
}
