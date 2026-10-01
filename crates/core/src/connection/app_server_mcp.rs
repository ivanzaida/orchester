//! Checks that project-selected MCP servers survived normal Codex profile loading. Global servers
//! remain available. A step that asks before MCP actions needs every selected server connected before
//! its turn begins ([`verify_selected_servers`]); any other step works without a server that is not,
//! as it did when Codex ran without App Server, and the run's activity says so
//! ([`unavailable_selected`]).

use std::collections::BTreeSet;

use serde_json::Value;
use thiserror::Error;

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
