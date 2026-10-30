//! What a Codex App Server `config/read` result (with its layers) says about the configuration an agent
//! would get. The repository's own `.codex` folder is never loaded: the invocation marks the worktree
//! and its repository untrusted (`crate::connection::invocation::CodexInvocation`), Codex then lists
//! the folder as a disabled project layer, and a project layer Codex loaded all the same refuses the
//! step ([`repository_settings`]).
//!
//! A server of the user's own configuration named like a server the step chooses would merge with it (Codex merges
//! the tables of its layers: the user's `env`, `args` or `command` would reach the chosen server, and a `url` of
//! theirs next to a chosen `command` makes Codex refuse its whole configuration). So before the step's Codex starts,
//! the names Codex's configuration has are read with a Codex App Server started for that alone ([`names_read_command`],
//! [`read_configured_names`]), and a chosen server named like one of them gets another name in Codex
//! (`crate::connection::invocation::CodexMcp::named_apart`). Should a name still be taken when the step's Codex reads
//! its configuration (it changed in between), the step is refused ([`refuse_taken_names`]), as it is for a model
//! provider of the user's own named like the one an API provider connection runs on ([`refuse_taken_provider`]): its
//! headers or query would merge into the requests that carry the provider's key.

use std::{collections::BTreeSet, path::Path};

use serde_json::{Value, json};
use thiserror::Error;

use crate::command::{Command, CommandError, LineExchange};

/// The id of the `config/read` request, the one the step's own Codex session asks it with.
const READ_REQUEST: u64 = 6;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ConfigReadError {
  #[error("Codex App Server did not report the layers of its configuration")]
  MissingLayers,
  #[error("Codex loaded the repository's own Codex settings in {0}, which Orchester never gives an agent")]
  RepositorySettingsLoaded(String),
  /// Orchester's own server, which keeps its name, or a server the step chooses whose name the configuration did not
  /// have when the step's servers were named ([`read_configured_names`]).
  #[error(
    "Your own Codex configuration has an MCP server named `{0}`, the name a server Orchester gives this agent has in \
     Codex; Codex would merge them. Retry the step if the step chose that server, as its servers get names apart \
     from yours when it starts; for Orchester's own server, rename yours"
  )]
  ServerNameTaken(String),
  #[error(
    "Your own Codex configuration has a model provider named `{0}`, the one Orchester runs this connection's API \
     provider as; Codex would merge them, so rename yours"
  )]
  ProviderNameTaken(String),
  #[error("Codex App Server did not answer the read of its configuration")]
  NoAnswer,
  #[error("Codex App Server refused to read its configuration: {0}")]
  Refused(String),
  #[error("Codex App Server did not report its configuration")]
  MissingConfiguration,
  #[error(transparent)]
  Command(#[from] CommandError),
}

/// `command`, a Codex App Server's without any server of the step's (`CodexInvocation` without `mcp`, on the step's
/// account), asked `initialize`, `initialized` and `config/read` for `worktree`; its input stays open until the
/// answer is in, as App Server stops at the end of its input without answering what it still works on.
pub fn names_read_command(command: Command, worktree: &Path) -> Command {
  let requests = [
    json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"orchester","version":"0.1"}}}),
    json!({"method":"initialized"}),
    json!({"id":READ_REQUEST,"method":"config/read","params":{"cwd":worktree.to_string_lossy()}}),
  ];
  let input: String = requests.iter().map(|request| format!("{request}\n")).collect();
  command.with_interactive_input(input)
}

/// The names of the MCP servers Codex's configuration has, read with `command` ([`names_read_command`]) through
/// `runner`.
pub fn read_configured_names(
  runner: &impl LineExchange,
  command: &Command,
) -> Result<BTreeSet<String>, ConfigReadError> {
  let lines = runner.exchange(command, &mut |line| answer(line).is_some())?;
  let answer = lines
    .iter()
    .find_map(|line| answer(line))
    .ok_or(ConfigReadError::NoAnswer)?;
  if let Some(error) = answer.get("error") {
    let message = error.get("message").and_then(Value::as_str).unwrap_or_default();
    return Err(ConfigReadError::Refused(
      message.split_whitespace().collect::<Vec<_>>().join(" "),
    ));
  }
  let config = answer
    .pointer("/result/config")
    .ok_or(ConfigReadError::MissingConfiguration)?;
  match config.get("mcp_servers") {
    None | Some(Value::Null) => Ok(BTreeSet::new()),
    Some(Value::Object(servers)) => Ok(servers.keys().cloned().collect()),
    Some(_) => Err(ConfigReadError::MissingConfiguration),
  }
}

/// The answer to the read in `line`, if it is that.
fn answer(line: &str) -> Option<Value> {
  serde_json::from_str::<Value>(line.trim()).ok().filter(|message| {
    message.get("id").and_then(Value::as_u64) == Some(READ_REQUEST) && message.get("method").is_none()
  })
}

/// Refuses a `config/read` result whose own layers (the user's, the system's: every loaded layer but
/// the arguments Orchester passed) define an MCP server named like one of `selected`, the names Codex gets the
/// step's servers by.
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
