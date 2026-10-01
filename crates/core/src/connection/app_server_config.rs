//! What a Codex App Server `config/read` result (with its layers) says about the configuration an agent
//! would get. The repository's own `.codex` folder is never loaded: the invocation marks the worktree
//! and its repository untrusted (`crate::connection::invocation::CodexInvocation`), Codex then lists
//! the folder as a disabled project layer, and a project layer Codex loaded all the same refuses the
//! step ([`repository_settings`]).

use serde_json::Value;
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ConfigReadError {
  #[error("Codex App Server did not report the layers of its configuration")]
  MissingLayers,
  #[error("Codex loaded the repository's own Codex settings in {0}, which Orchester never gives an agent")]
  RepositorySettingsLoaded(String),
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
