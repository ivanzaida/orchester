//! Checks that project-selected MCP servers survived normal Codex profile loading. Global
//! servers remain available; a selected server must be connected before the turn begins.

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

pub fn verify_selected_servers(result: &Value, selected: &BTreeSet<String>) -> Result<(), McpInventoryError> {
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
  if !selected.is_subset(&names) {
    return Err(McpInventoryError::MissingSelected);
  }
  for server in data {
    let Some(name) = server.get("name").and_then(Value::as_str) else {
      continue;
    };
    if selected.contains(name)
      && (server.get("runtimeStatus").and_then(Value::as_str) != Some("connected")
        || server.get("toolsError").is_some_and(|error| !error.is_null()))
    {
      return Err(McpInventoryError::UnavailableSelected);
    }
  }
  Ok(())
}
