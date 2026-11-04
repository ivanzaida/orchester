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
//!
//! The Orchestrator chat's read also lists Codex's skills ([`chat_read_command`], [`read_chat_configuration`]), so that
//! its turn disables the person's own (ORC-663): Codex 0.166.0 loads `$CODEX_HOME/skills` whatever the isolation
//! overrides say, and `skills.config` disables a skill by its path.

use std::{
  collections::BTreeSet,
  path::{Path, PathBuf},
};

use serde_json::{Value, json};
use thiserror::Error;

use crate::command::{Command, CommandError, LineExchange};

/// The id of the `config/read` request, the one the step's own Codex session asks it with.
const READ_REQUEST: u64 = 6;
/// The id of the chat's `skills/list` request.
const SKILLS_REQUEST: u64 = 7;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ConfigReadError {
  #[error("Codex App Server did not report the layers of its configuration")]
  MissingLayers,
  #[error("Codex loaded the repository's own Codex settings in {0}, which Orchester never gives an agent")]
  RepositorySettingsLoaded(String),
  /// The runs' relay of Orchester's tools (`Orchester`), which keeps its name, or a server Orchester names apart (a
  /// step's chosen one, the Orchestrator chat's `orchester`) whose name the configuration did not have when the
  /// servers were named ([`read_configured_names`]). Connect an agent registers the app's server as `orchester`, never
  /// as `Orchester` (`docs/design/app-mcp-connection.md`, "The server name").
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
  #[error("Codex App Server did not answer the list of its skills")]
  NoSkillsAnswer,
  #[error("Codex App Server refused to list its skills: {0}")]
  SkillsRefused(String),
  #[error("Codex App Server did not report its skills")]
  MissingSkills,
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
  configured_names(&lines)
}

/// What the Orchestrator chat reads of Codex's own setup before a turn on Codex (ORC-663): the names of the MCP
/// servers its configuration has ([`read_configured_names`]) and the person's own skills, which the turn disables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatConfiguration {
  pub names: BTreeSet<String>,
  /// The `SKILL.md` of every enabled skill Codex lists for the project folder but its bundled ones (`system`): the
  /// person's (`user`, from `$CODEX_HOME/skills` and `~/.agents/skills`), an administrator's (`admin`) and any
  /// repository skill the runner's own search did not disable (`repo`).
  pub own_skills: Vec<PathBuf>,
}

/// `command` as [`names_read_command`] asks it, then also `skills/list` of `folder`, read afresh.
pub fn chat_read_command(command: Command, folder: &Path) -> Command {
  let requests = [
    json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"orchester","version":"0.1"}}}),
    json!({"method":"initialized"}),
    json!({"id":READ_REQUEST,"method":"config/read","params":{"cwd":folder.to_string_lossy()}}),
    json!({"id":SKILLS_REQUEST,"method":"skills/list","params":{"cwds":[folder.to_string_lossy()],"forceReload":true}}),
  ];
  let input: String = requests.iter().map(|request| format!("{request}\n")).collect();
  command.with_interactive_input(input)
}

/// The names and skills `command` ([`chat_read_command`]) reads through `runner`: both answers, or why not.
pub fn read_chat_configuration(
  runner: &impl LineExchange,
  command: &Command,
) -> Result<ChatConfiguration, ConfigReadError> {
  let mut waiting = BTreeSet::from([READ_REQUEST, SKILLS_REQUEST]);
  let lines = runner.exchange(command, &mut |line| {
    if let Some(id) = response_id(line) {
      waiting.remove(&id);
    }
    waiting.is_empty()
  })?;
  let names = configured_names(&lines)?;
  let skills = lines
    .iter()
    .find_map(|line| response(line, SKILLS_REQUEST))
    .ok_or(ConfigReadError::NoSkillsAnswer)?;
  if let Some(error) = skills.get("error") {
    return Err(ConfigReadError::SkillsRefused(refusal(error)));
  }
  let own_skills = own_skills(skills.get("result").unwrap_or(&Value::Null))?;
  Ok(ChatConfiguration { names, own_skills })
}

/// The skills of a `skills/list` result to disable: every enabled one but Codex's bundled ones, each once.
pub fn own_skills(result: &Value) -> Result<Vec<PathBuf>, ConfigReadError> {
  let entries = result
    .get("data")
    .and_then(Value::as_array)
    .ok_or(ConfigReadError::MissingSkills)?;
  let mut found = Vec::new();
  for skill in entries
    .iter()
    .filter_map(|entry| entry.get("skills").and_then(Value::as_array))
    .flatten()
  {
    let enabled = skill.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let bundled = skill.get("scope").and_then(Value::as_str) == Some("system");
    let path = skill
      .get("path")
      .and_then(Value::as_str)
      .ok_or(ConfigReadError::MissingSkills)?;
    let path = PathBuf::from(path);
    if enabled && !bundled && !found.contains(&path) {
      found.push(path);
    }
  }
  Ok(found)
}

/// The names of the MCP servers the `config/read` answer among `lines` reports.
fn configured_names(lines: &[String]) -> Result<BTreeSet<String>, ConfigReadError> {
  let answer = lines
    .iter()
    .find_map(|line| answer(line))
    .ok_or(ConfigReadError::NoAnswer)?;
  if let Some(error) = answer.get("error") {
    return Err(ConfigReadError::Refused(refusal(error)));
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

/// Codex's message in `error`, on one line.
fn refusal(error: &Value) -> String {
  let message = error.get("message").and_then(Value::as_str).unwrap_or_default();
  message.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The answer to the read in `line`, if it is that.
fn answer(line: &str) -> Option<Value> {
  response(line, READ_REQUEST)
}

/// The answer to request `id` in `line`, if it is that.
fn response(line: &str, id: u64) -> Option<Value> {
  serde_json::from_str::<Value>(line.trim())
    .ok()
    .filter(|message| response_id_of(message) == Some(id))
}

/// The id of the request `line` answers, if it answers one.
fn response_id(line: &str) -> Option<u64> {
  serde_json::from_str::<Value>(line.trim())
    .ok()
    .and_then(|message| response_id_of(&message))
}

fn response_id_of(message: &Value) -> Option<u64> {
  message
    .get("method")
    .is_none()
    .then(|| message.get("id").and_then(Value::as_u64))
    .flatten()
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
