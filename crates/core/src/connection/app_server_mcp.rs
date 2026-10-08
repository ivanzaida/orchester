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
//! A step that asks before MCP actions approves the tools its servers list to run without asking:
//! `mcp_servers.<server>.tools.<tool>.approval_mode = "approve"`. Codex 0.159.2 then runs that tool
//! without an approval request, while the server's other tools ask as before (checked 2026-10-02 with
//! turns against a local mock model: one tool set to `approve` ran unasked while another tool of the
//! same server sent `mcpServer/elicitation/request`; `config/read` lists the setting, and an unknown mode
//! is refused with *unknown variant*, expecting `auto`, `prompt`, `writes` or `approve`). A tool not
//! listed keeps Codex's default mode (`auto`). The approval is an override of the Codex process, next to
//! the server's own (`crate::connection::invocation::CodexMcp`), never part of the thread's configuration
//! ([`thread_config`]).
//!
//! The thread's configuration names each server it turns off by a dotted key,
//! `mcp_servers.<server>.enabled`, never by an `mcp_servers` table: in Codex 0.159.2 a thread's
//! `mcp_servers` table, even an empty one, replaces the table of the process's overrides, so the step's
//! own servers and Orchester's are gone from the thread, while the user's servers merge with it (checked
//! 2026-10-08 with `config/read`, `thread/start` and `mcpServerStatus/list`). Codex runs a server whose name
//! has a dot (`[mcp_servers."a.b"]`), but splits a dotted key at every dot and reads no quotes in it:
//! `mcp_servers.a.b.enabled` refuses the thread with *invalid transport in `mcp_servers.a`*, and a name
//! like `<chosen>.x` would add a setting to a chosen server. Such a server cannot be turned off, so it
//! refuses the step before its thread starts ([`servers_to_turn_off`]); a server Orchester gives an agent
//! never has a dot in its name (`crate::connection::mcp::McpServerName`).

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum McpInventoryError {
  #[error("Codex App Server MCP inventory is too large to verify")]
  TooLarge,
  #[error("Codex App Server omitted MCP inventory")]
  MissingInventory,
  #[error("Codex App Server MCP inventory has an unnamed server")]
  UnnamedServer,
  #[error(
    "Your own Codex configuration has an MCP server named `{0}`; Orchester cannot keep a server whose name has a \
     dot from its agents, so rename it"
  )]
  DottedName(String),
  #[error("Codex App Server did not load the MCP server `{0}` selected by this agent profile")]
  MissingSelected(String),
  #[error(
    "The MCP server `{}` selected by this agent profile is not connected in Codex App Server: {}",
    .0.name,
    .0.detail().unwrap_or_default()
  )]
  UnavailableSelected(UnavailableServer),
  #[error("Codex App Server did not report its configuration")]
  MissingConfiguration,
  #[error("Codex App Server runs the MCP server `{0}`, which this step does not choose")]
  UnchosenServer(String),
  #[error("The Codex configuration defines hooks, which Orchester cannot keep from its agents")]
  UserHooks,
}

/// The configured MCP servers of a `config/read` result that `selected` does not name: the user's own
/// and a project's, which the thread turns off; an error for one whose name has a dot, which cannot be.
pub fn servers_to_turn_off(result: &Value, selected: &BTreeSet<String>) -> Result<Vec<String>, McpInventoryError> {
  let config = result.get("config").ok_or(McpInventoryError::MissingConfiguration)?;
  let servers = match config.get("mcp_servers") {
    None | Some(Value::Null) => return Ok(Vec::new()),
    Some(servers) => servers.as_object().ok_or(McpInventoryError::MissingConfiguration)?,
  };
  let turned_off: Vec<String> = servers
    .keys()
    .filter(|name| !selected.contains(*name))
    .cloned()
    .collect();
  if let Some(dotted) = turned_off.iter().find(|name| name.contains('.')) {
    return Err(McpInventoryError::DottedName(dotted.clone()));
  }
  Ok(turned_off)
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
  /// has no such server or no state.
  pub state: Option<String>,
  /// Codex's reason it has no tools of the server (`toolsError`), such as why the server failed to
  /// start, as the caller's `redact` left it: it is the server's own diagnostic and may hold what the
  /// server was given.
  pub error: Option<String>,
}

impl UnavailableServer {
  /// The state and Codex's reason, as `failed: <reason>`; `None` for a server the inventory does not
  /// list.
  pub fn detail(&self) -> Option<String> {
    let state = self.state.as_deref()?;
    Some(match &self.error {
      Some(error) => format!("{state}: {error}"),
      None => state.to_owned(),
    })
  }
}

/// Every selected server must be connected, or the step does not start; the error names the first
/// that is not, and why, Codex's reason passed through `redact`.
pub fn verify_selected_servers(
  result: &Value,
  selected: &BTreeSet<String>,
  redact: impl Fn(&str) -> String,
) -> Result<(), McpInventoryError> {
  match unavailable_selected(result, selected, redact)?.into_iter().next() {
    None => Ok(()),
    Some(UnavailableServer { name, state: None, .. }) => Err(McpInventoryError::MissingSelected(name)),
    Some(server) => Err(McpInventoryError::UnavailableSelected(server)),
  }
}

/// The selected servers of `result` (an `mcpServerStatus/list` result) that are not connected, the
/// missing ones first, each with Codex's reason passed through `redact`, which removes the invocation's
/// secret values; an error only when the inventory itself cannot be read.
pub fn unavailable_selected(
  result: &Value,
  selected: &BTreeSet<String>,
  redact: impl Fn(&str) -> String,
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
      error: None,
    })
    .collect();
  for server in data {
    let Some(name) = server.get("name").and_then(Value::as_str) else {
      continue;
    };
    let state = server.get("runtimeStatus").and_then(Value::as_str);
    let error = server.get("toolsError").filter(|error| !error.is_null());
    if selected.contains(name) && (state != Some("connected") || error.is_some()) {
      unavailable.push(UnavailableServer {
        name: name.to_owned(),
        state: Some(state.unwrap_or("unknown").to_owned()),
        error: error.map(|error| redact(&error.as_str().map_or_else(|| error.to_string(), str::to_owned))),
      });
    }
  }
  Ok(unavailable)
}

/// The thread's configuration: each server of `turned_off` turned off by its dotted key
/// (`mcp_servers.<server>.enabled = false`), which merges with the user's configuration and leaves the
/// process's own servers alone. An `mcp_servers` table would replace the process's servers for the
/// thread (see the module's notes). No name has a dot ([`servers_to_turn_off`]).
pub fn thread_config(turned_off: &[String]) -> Map<String, Value> {
  turned_off
    .iter()
    .map(|name| (format!("mcp_servers.{name}.enabled"), json!(false)))
    .collect()
}
