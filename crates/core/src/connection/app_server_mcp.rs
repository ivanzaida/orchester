//! The MCP servers of a Codex App Server thread. App Server loads the user's own configuration, and a
//! trusted project's `.codex` folders, with their servers; the agent gets none of them: each is turned
//! off for the thread by name ([`servers_to_turn_off`], read from `config/read`), and the thread's
//! inventory must show every server it does not choose turned off ([`verify_only_chosen_run`]). A step
//! that asks before MCP actions needs every selected server connected before its turn begins
//! ([`verify_selected_servers`]); any other step works without a server that is not, and the run's
//! activity says so ([`unavailable_selected`]). User hooks would run programs of the user's setup
//! with the agent's work, and nothing turns them off, so a configuration with any refuses the step
//! ([`refuse_user_hooks`]).
//!
//! A step that asks before MCP actions approves the tools its servers list to run without asking in
//! the thread's configuration ([`thread_servers`]): `mcp_servers.<server>.tools.<tool>.approval_mode =
//! "approve"`. Codex 0.159.2 then runs that tool without an approval request, while the server's other
//! tools ask as before (checked 2026-10-02 with turns against a local mock model: one tool set to
//! `approve` ran unasked while another tool of the same server sent `mcpServer/elicitation/request`;
//! `config/read` lists the setting, and an unknown mode is refused with *unknown variant*, expecting
//! `auto`, `prompt`, `writes` or `approve`). A tool not listed keeps Codex's default mode (`auto`).

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};
use thiserror::Error;

use super::mcp::InvocationExemptions;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum McpInventoryError {
  #[error("Codex App Server MCP inventory is too large to verify")]
  TooLarge,
  #[error("Codex App Server omitted MCP inventory")]
  MissingInventory,
  #[error("Codex App Server MCP inventory has an unnamed server")]
  UnnamedServer,
  #[error("Codex App Server omitted an MCP server selected by this agent profile")]
  MissingSelected,
  #[error("A selected MCP server is not connected in Codex App Server")]
  UnavailableSelected,
  #[error("Codex App Server did not report its configuration")]
  MissingConfiguration,
  #[error("Codex App Server runs the MCP server `{0}`, which this step does not choose")]
  UnchosenServer(String),
  #[error("The Codex configuration defines hooks, which Orchester cannot keep from its agents")]
  UserHooks,
}

/// The configured MCP servers of a `config/read` result that `selected` does not name: the user's own
/// and a project's, which the thread turns off.
pub fn servers_to_turn_off(result: &Value, selected: &BTreeSet<String>) -> Result<Vec<String>, McpInventoryError> {
  let config = result.get("config").ok_or(McpInventoryError::MissingConfiguration)?;
  let servers = match config.get("mcp_servers") {
    None | Some(Value::Null) => return Ok(Vec::new()),
    Some(servers) => servers.as_object().ok_or(McpInventoryError::MissingConfiguration)?,
  };
  Ok(
    servers
      .keys()
      .filter(|name| !selected.contains(*name))
      .cloned()
      .collect(),
  )
}

/// Refuses a `config/read` result whose configuration defines hooks.
pub fn refuse_user_hooks(result: &Value) -> Result<(), McpInventoryError> {
  let hooks = result.get("config").and_then(|config| config.get("hooks"));
  let defined = match hooks {
    None | Some(Value::Null) => false,
    Some(Value::Object(table)) => !table.is_empty(),
    Some(Value::Array(list)) => !list.is_empty(),
    Some(_) => true,
  };
  if defined {
    return Err(McpInventoryError::UserHooks);
  }
  Ok(())
}

/// Every server of an `mcpServerStatus/list` result that `selected` does not name must be turned off.
pub fn verify_only_chosen_run(result: &Value, selected: &BTreeSet<String>) -> Result<(), McpInventoryError> {
  let data = result
    .get("data")
    .and_then(Value::as_array)
    .ok_or(McpInventoryError::MissingInventory)?;
  for server in data {
    let name = server
      .get("name")
      .and_then(Value::as_str)
      .ok_or(McpInventoryError::UnnamedServer)?;
    if !selected.contains(name) && server.get("runtimeStatus").and_then(Value::as_str) != Some("disabled") {
      return Err(McpInventoryError::UnchosenServer(name.to_owned()));
    }
  }
  Ok(())
}

/// A selected server Codex has no connection to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnavailableServer {
  pub name: String,
  /// Its connection state as Codex reports it (`failed`, `notStarted`, …); `None` when the inventory
  /// has no such server or no state. Codex's tool discovery error is left out: it is the server's own
  /// diagnostic and may hold what the server was given.
  pub state: Option<String>,
}

/// Every selected server must be connected, or the step does not start.
pub fn verify_selected_servers(result: &Value, selected: &BTreeSet<String>) -> Result<(), McpInventoryError> {
  match unavailable_selected(result, selected)?.first() {
    None => Ok(()),
    Some(UnavailableServer { state: None, .. }) => Err(McpInventoryError::MissingSelected),
    Some(_) => Err(McpInventoryError::UnavailableSelected),
  }
}

/// The selected servers of `result` (an `mcpServerStatus/list` result) that are not connected, the
/// missing ones first; an error only when the inventory itself cannot be read.
pub fn unavailable_selected(
  result: &Value,
  selected: &BTreeSet<String>,
) -> Result<Vec<UnavailableServer>, McpInventoryError> {
  if result.get("nextCursor").is_some_and(|cursor| !cursor.is_null()) {
    return Err(McpInventoryError::TooLarge);
  }
  let data = result
    .get("data")
    .and_then(Value::as_array)
    .ok_or(McpInventoryError::MissingInventory)?;
  let names: Option<BTreeSet<_>> = data
    .iter()
    .map(|server| server.get("name").and_then(Value::as_str).map(str::to_owned))
    .collect();
  let names = names.ok_or(McpInventoryError::UnnamedServer)?;
  let mut unavailable: Vec<UnavailableServer> = selected
    .difference(&names)
    .map(|name| UnavailableServer {
      name: name.clone(),
      state: None,
    })
    .collect();
  for server in data {
    let Some(name) = server.get("name").and_then(Value::as_str) else {
      continue;
    };
    let state = server.get("runtimeStatus").and_then(Value::as_str);
    if selected.contains(name)
      && (state != Some("connected") || server.get("toolsError").is_some_and(|error| !error.is_null()))
    {
      unavailable.push(UnavailableServer {
        name: name.to_owned(),
        state: Some(state.unwrap_or("unknown").to_owned()),
      });
    }
  }
  Ok(unavailable)
}

/// The `mcp_servers` table of a thread's configuration: each server of `turned_off` turned off by name
/// (a table merges with the user's configuration, so `mcp_servers={}` would not remove them), and each
/// tool of `exempt` approved, so that Codex runs it without asking.
pub fn thread_servers(turned_off: &[String], exempt: &InvocationExemptions) -> Map<String, Value> {
  let mut servers: Map<String, Value> = turned_off
    .iter()
    .map(|name| (name.clone(), json!({"enabled": false})))
    .collect();
  for (server, tools) in exempt.servers() {
    let tools: Map<String, Value> = tools
      .tools()
      .iter()
      .map(|tool| (tool.to_string(), json!({"approval_mode": "approve"})))
      .collect();
    servers.insert(server.to_string(), json!({"tools": tools}));
  }
  servers
}
