//! What a Codex App Server `config/read` result (with its layers) says about the configuration an agent
//! would get. The repository's own `.codex` folder is never loaded: the invocation marks the worktree
//! and its repository untrusted (`crate::connection::invocation::CodexInvocation`), Codex then lists
//! the folder as a disabled project layer, and a project layer Codex loaded all the same refuses the
//! step ([`repository_settings`]). A server of the user's own configuration named like a server the
//! step chooses would merge with it (Codex merges the tables of its layers: the user's `env`, `args` or
//! `command` would reach the chosen server), so that refuses the step too ([`refuse_taken_names`]), as does a model
//! provider of the user's own named like the one an API provider connection runs on ([`refuse_taken_provider`]): its
//! headers or query would merge into the requests that carry the provider's key.

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
  #[error(
    "Your own Codex configuration has a model provider named `{0}`, the one Orchester runs this connection's API \
     provider as; Codex would merge them, so rename yours"
  )]
  ProviderNameTaken(String),
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

/// Refuses a `config/read` result whose own layers define a model provider with the id `provider`.
pub fn refuse_taken_provider(result: &Value, provider: &str) -> Result<(), ConfigReadError> {
  let layers = result
    .get("layers")
    .and_then(Value::as_array)
    .ok_or(ConfigReadError::MissingLayers)?;
  let taken = layers
    .iter()
    .filter(|layer| {
      let kind = layer.pointer("/name/type").and_then(Value::as_str);
      kind != Some("sessionFlags") && layer.get("disabledReason").is_none_or(Value::is_null)
    })
    .any(|layer| {
      layer
        .pointer("/config/model_providers")
        .and_then(Value::as_object)
        .is_some_and(|providers| providers.contains_key(provider))
    });
  if taken {
    return Err(ConfigReadError::ProviderNameTaken(provider.to_owned()));
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
