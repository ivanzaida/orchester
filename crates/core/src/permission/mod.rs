//! What an agent may do while it works on a task (`docs/design/orchester-workspace.md`, "Agent
//! permissions"): repository access, which commands it may run and whether it has network access.
//! Permissions belong to an agent profile version; a pipeline step can only narrow them
//! ([`StepPermissions`]). Which options a connection's CLI can enforce is decided by
//! [`capability`]; an option it cannot enforce is never offered, and Orchester never starts an agent
//! in a bypass mode.

pub mod capability;
pub mod claude;
mod narrow;

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::narrow::{StepCommands, StepNetwork, StepPermissions};
pub use crate::resource::RepositoryAccess;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CommandPrefixError {
  #[error("a command must not be blank")]
  Blank,
  #[error("a command is at most {max} characters long", max = CommandPrefix::MAX_CHARS)]
  TooLong,
  /// Characters that chain, redirect or substitute commands, or that the CLIs' rule syntax uses.
  #[error("a command must not contain `{0}`")]
  ForbiddenCharacter(char),
}

/// A command an agent may run, matched by prefix on whole words: `pnpm test` also allows
/// `pnpm test src/auth`, not `pnpm tester`. Words are separated by single spaces.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommandPrefix(String);

impl CommandPrefix {
  pub const MAX_CHARS: usize = 120;
  const FORBIDDEN: &'static [char] = &[';', '&', '|', '<', '>', '`', '$', '(', ')', '*', '\\'];

  pub fn new(value: &str) -> Result<Self, CommandPrefixError> {
    let words: Vec<&str> = value.split_whitespace().collect();
    if words.is_empty() {
      return Err(CommandPrefixError::Blank);
    }
    let prefix = words.join(" ");
    if let Some(c) = prefix.chars().find(|c| c.is_control() || Self::FORBIDDEN.contains(c)) {
      return Err(CommandPrefixError::ForbiddenCharacter(c));
    }
    if prefix.chars().count() > Self::MAX_CHARS {
      return Err(CommandPrefixError::TooLong);
    }
    Ok(Self(prefix))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// Whether this prefix allows `other`: the same words, or `other` continues them.
  pub fn covers(&self, other: &Self) -> bool {
    other.0 == self.0 || other.0.strip_prefix(&self.0).is_some_and(|rest| rest.starts_with(' '))
  }
}

impl fmt::Display for CommandPrefix {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for CommandPrefix {
  type Error = CommandPrefixError;

  fn try_from(value: String) -> Result<Self, CommandPrefixError> {
    Self::new(&value)
  }
}

impl From<CommandPrefix> for String {
  fn from(prefix: CommandPrefix) -> Self {
    prefix.0
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AllowedCommandsError {
  #[error("`{0}` is listed more than once")]
  Duplicate(CommandPrefix),
  #[error("at most {max} commands can be listed", max = AllowedCommands::MAX_ENTRIES)]
  TooMany,
}

/// The commands an agent may run, in the order they were added. Empty allows no command.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Vec<CommandPrefix>", into = "Vec<CommandPrefix>")]
pub struct AllowedCommands(Vec<CommandPrefix>);

impl AllowedCommands {
  pub const MAX_ENTRIES: usize = 50;

  pub fn new(entries: Vec<CommandPrefix>) -> Result<Self, AllowedCommandsError> {
    if entries.len() > Self::MAX_ENTRIES {
      return Err(AllowedCommandsError::TooMany);
    }
    for (index, entry) in entries.iter().enumerate() {
      if entries[..index].contains(entry) {
        return Err(AllowedCommandsError::Duplicate(entry.clone()));
      }
    }
    Ok(Self(entries))
  }

  pub fn entries(&self) -> &[CommandPrefix] {
    &self.0
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// Whether some entry allows `command`.
  pub fn allows(&self, command: &CommandPrefix) -> bool {
    self.0.iter().any(|entry| entry.covers(command))
  }

  /// The entries of `self` that `wider` allows, for narrowing.
  fn within(&self, wider: &Self) -> Self {
    Self(self.0.iter().filter(|entry| wider.allows(entry)).cloned().collect())
  }
}

impl TryFrom<Vec<CommandPrefix>> for AllowedCommands {
  type Error = AllowedCommandsError;

  fn try_from(entries: Vec<CommandPrefix>) -> Result<Self, AllowedCommandsError> {
    Self::new(entries)
  }
}

impl From<AllowedCommands> for Vec<CommandPrefix> {
  fn from(commands: AllowedCommands) -> Self {
    commands.0
  }
}

/// Which commands an agent may run.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "commands")]
pub enum CommandPermission {
  /// Every command waits for a person's decision. Needs the permission-request flow, which does
  /// not exist yet, so no connection can enforce it (see [`capability`]).
  AskBefore,
  /// Only the listed commands run; anything else is denied and the agent is told why.
  Allowed(AllowedCommands),
  /// Any command, confined to the task's worktree.
  AnyInWorktree,
}

/// Whether the agent's commands and tools may reach the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkAccess {
  Off,
  On,
}

/// The permissions of an agent profile version, or the effective permissions of a step.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentPermissions {
  repository: RepositoryAccess,
  commands: CommandPermission,
  network: NetworkAccess,
}

impl AgentPermissions {
  pub const fn new(repository: RepositoryAccess, commands: CommandPermission, network: NetworkAccess) -> Self {
    Self {
      repository,
      commands,
      network,
    }
  }

  /// What profiles saved before permissions existed get: read-only, commands confined to the
  /// worktree (which read-only access limits to reading it), network off. Commands are "any in
  /// the worktree" rather than an empty list because that is what those profiles ran with, and
  /// the Codex CLI, the only one that runs today, cannot enforce a list.
  pub const fn before_permissions() -> Self {
    Self::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off,
    )
  }

  pub const fn repository(&self) -> RepositoryAccess {
    self.repository
  }

  pub const fn commands(&self) -> &CommandPermission {
    &self.commands
  }

  pub const fn network(&self) -> NetworkAccess {
    self.network
  }

  pub const fn set_repository(&mut self, repository: RepositoryAccess) {
    self.repository = repository;
  }

  pub fn set_commands(&mut self, commands: CommandPermission) {
    self.commands = commands;
  }

  pub const fn set_network(&mut self, network: NetworkAccess) {
    self.network = network;
  }
}
