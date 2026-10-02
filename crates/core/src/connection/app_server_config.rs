//! What a Codex App Server `config/read` result (with its layers) says about the configuration an agent
//! would get. The repository's own `.codex` folder is never loaded: the invocation marks the worktree
//! and its repository untrusted (`crate::connection::invocation::CodexInvocation`), Codex then lists
//! the folder as a disabled project layer, and a project layer Codex loaded all the same refuses the
//! step ([`repository_settings`]). A server of the user's own configuration named like a server the
//! step chooses would merge with it (Codex merges the tables of its layers: the user's `env`, `args` or
//! `command` would reach the chosen server), so that refuses the step too ([`refuse_taken_names`]).

use std::collections::BTreeSet;

use serde_json::Value;
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ConfigReadError {
  #[error("Codex App Server did not report the layers of its configuration")]
  MissingLayers,
  #[error("Codex loaded the repository's own Codex settings in {0}, which Orchester never gives an agent")]
  RepositorySettingsLoaded(String),
  #[error(
    "Your own Codex configuration has an MCP server named `{0}`, like a server this step chooses; Codex would merge \
     them, so rename one of them"
  )]
  ServerNameTaken(String),
}

/// Refuses a `config/read` result whose own layers (the user's, the system's: every loaded layer but
/// the arguments Orchester passed) define an MCP server named like one of `selected`.
pub fn refuse_taken_names(result: &Value, selected: &BTreeSet<String>) -> Result<(), ConfigReadError> {
  let layers = result
    .get("layers")
    .and_then(Value::as_array)
    .ok_or(ConfigReadError::MissingLayers)?;
  let own = layers.iter().filter(|layer| {
    let kind = layer.pointer("/name/type").and_then(Value::as_str);
    kind != Some("sessionFlags") && layer.get("disabledReason").is_none_or(Value::is_null)
  });
  for layer in own {
    let Some(servers) = layer.pointer("/config/mcp_servers").and_then(Value::as_object) else {
      continue;
    };
    if let Some(name) = servers.keys().find(|name| selected.contains(*name)) {
      return Err(ConfigReadError::ServerNameTaken(name.clone()));
    }
  }
  Ok(())
}

/// The repository's `.codex` folder Codex left out, if the worktree has one; an error when Codex
/// loaded a project layer.
pub fn repository_settings(result: &Value) -> Result<Option<String>, ConfigReadError> {
  let layers = result
    .get("layers")
    .and_then(Value::as_array)
    .ok_or(ConfigReadError::MissingLayers)?;
  let mut ignored = None;
  for layer in layers {
    let name = layer.get("name");
    if name.and_then(|name| name.get("type")).and_then(Value::as_str) != Some("project") {
      continue;
    }
    let folder = name
      .and_then(|name| name.get("dotCodexFolder"))
      .and_then(Value::as_str)
      .unwrap_or_default()
      .to_owned();
    if layer.get("disabledReason").is_none_or(Value::is_null) {
      return Err(ConfigReadError::RepositorySettingsLoaded(folder));
    }
    ignored.get_or_insert(folder);
  }
  Ok(ignored)
}
