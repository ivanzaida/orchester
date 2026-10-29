//! What an agent may do while it works on a task (`docs/design/orchester-workspace.md`, "Agent
//! permissions"): repository access, the folders outside the worktree it may also write to
//! ([`writable`]), which commands it may run and whether it has network access.
//! Permissions belong to an agent profile version; a pipeline step can only narrow them
//! ([`StepPermissions`]). Which options a connection's CLI can enforce is decided by
//! [`capability`]; an option it cannot enforce is never offered, and Orchester never starts an agent
//! in a bypass mode.

mod approval_mode;
pub(crate) mod canonical;
pub mod capability;
pub mod category;
pub mod claude;
pub mod known_safe;
mod launch;
pub(crate) mod link_targets;
mod local_file;
pub mod mcp;
mod narrow;
#[cfg(windows)]
pub(crate) mod remote_drive;
pub mod roots;
pub mod widening;
pub mod writable;

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  approval_mode::ApprovalMode,
  launch::{LaunchPermissions, OutsideCommands},
  local_file::{ProgramPlace, program_place, resolve_local, stays_local},
  narrow::{StepCommands, StepNetwork, StepPermissions},
  writable::{
    FolderBounds, ResolvedFolder, WritableFolder, WritableFolderError, WritableFolders, WritableFoldersError,
  },
};
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

/// Commands that create, copy, move, remove or edit files; Claude Code's `acceptEdits` runs exactly
/// these without a rule ([`claude`]).
const FILE_COMMANDS: [&str; 7] = ["mkdir", "touch", "rm", "rmdir", "mv", "cp", "sed"];

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

  /// Whether the first word creates, copies, moves, removes or edits files (`mkdir`, `touch`, `rm`,
  /// `rmdir`, `mv`, `cp`, `sed`). As every prefix, it also allows any arguments after it, which for
  /// these can change other files than the listed form names (`rm -f build/x ../src`).
  pub fn starts_with_file_command(&self) -> bool {
    self
      .0
      .split(' ')
      .next()
      .is_some_and(|first| FILE_COMMANDS.contains(&first))
  }

  /// Whether this prefix allows the program and arguments `argv`: its words are the first elements of
  /// `argv`, each whole. An argument with spaces matches no word, so `git "status --short"` is not
  /// `git status`, while arguments after the prefix may hold anything (`git commit -m "a message"`).
  /// Cargo's toolchain selector right after the program (`cargo +nightly fmt`) chooses which Cargo runs,
  /// not what it does, so `cargo fmt` allows it too, unless the prefix names a toolchain itself.
  pub fn covers_argv(&self, argv: &[String]) -> bool {
    let words: Vec<&str> = self.0.split(' ').collect();
    let argv = match (argv, words.get(1)) {
      ([program, toolchain, rest @ ..], Some(second))
        if program == "cargo" && toolchain.starts_with('+') && !second.starts_with('+') && !rest.is_empty() =>
      {
        std::iter::once(program).chain(rest).cloned().collect()
      }
      _ => argv.to_vec(),
    };
    argv.len() >= words.len() && words.iter().zip(&argv).all(|(word, arg)| *word == arg)
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

  /// Whether some entry allows the program and arguments `argv` ([`CommandPrefix::covers_argv`]).
  pub fn allows_argv(&self, argv: &[String]) -> bool {
    self.0.iter().any(|entry| entry.covers_argv(argv))
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
  /// Codex routes commands requiring approval to a person's one-action decision.
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
  /// Apply only with [`RepositoryAccess::Write`]; kept while the access is read-only so that
  /// switching back restores them. Left out of the stored form while empty, as before they existed.
  #[serde(default, skip_serializing_if = "WritableFolders::is_empty")]
  writable_folders: WritableFolders,
  commands: CommandPermission,
  network: NetworkAccess,
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  interactive_tool_approvals: bool,
  /// *Approve automatically (don't ask me)*: Orchester answers what would ask a person itself, but for
  /// what auto-approval never approves ([`crate::run::auto_verdict`]). Left out of the stored form while
  /// off, as before it existed.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  auto_approve: bool,
  /// Left out of the stored form while Manual, so every profile saved before approval modes existed reads as
  /// Manual with its commands as they were ([`crate::resource::AgentProfile`]'s schema 5).
  #[serde(default, skip_serializing_if = "ApprovalMode::is_manual")]
  approval_mode: ApprovalMode,
}

impl AgentPermissions {
  /// Manual, the stricter approval mode; see [`Self::set_approval_mode`].
  pub const fn new(repository: RepositoryAccess, commands: CommandPermission, network: NetworkAccess) -> Self {
    Self {
      repository,
      writable_folders: WritableFolders::none(),
      commands,
      network,
      interactive_tool_approvals: false,
      auto_approve: false,
      approval_mode: ApprovalMode::Manual,
    }
  }

  /// These permissions with the extra writable folders `folders`.
  #[must_use]
  pub fn with_writable_folders(mut self, folders: WritableFolders) -> Self {
    self.writable_folders = folders;
    self
  }

  /// What profiles saved before permissions existed get: read-only, commands confined to the
  /// worktree (which read-only access limits to reading it), network off. Commands are "any in
  /// the worktree" rather than an empty list because that is what those profiles ran with: only the
  /// Codex CLI ran then, and it cannot enforce a list.
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

  /// The extra writable folders as chosen, whatever the repository access.
  pub const fn writable_folders(&self) -> &WritableFolders {
    &self.writable_folders
  }

  /// The extra writable folders an agent gets: `None` unless it edits files.
  pub const fn granted_folders(&self) -> Option<&WritableFolders> {
    match self.repository {
      RepositoryAccess::Write => Some(&self.writable_folders),
      RepositoryAccess::ReadOnly => None,
    }
  }

  pub const fn commands(&self) -> &CommandPermission {
    &self.commands
  }

  pub const fn network(&self) -> NetworkAccess {
    self.network
  }

  pub const fn interactive_tool_approvals(&self) -> bool {
    self.interactive_tool_approvals
  }

  pub const fn set_interactive_tool_approvals(&mut self, enabled: bool) {
    self.interactive_tool_approvals = enabled;
  }

  pub const fn auto_approve(&self) -> bool {
    self.auto_approve
  }

  pub const fn set_auto_approve(&mut self, enabled: bool) {
    self.auto_approve = enabled;
  }

  pub const fn approval_mode(&self) -> ApprovalMode {
    self.approval_mode
  }

  /// Switching to Autopilot widens ([`widening::Widening::Autopilot`]); switching to Manual never does. Either way
  /// the allowed commands stay as they are: Manual runs them at once, Autopilot needs no list.
  pub const fn set_approval_mode(&mut self, mode: ApprovalMode) {
    self.approval_mode = mode;
  }
  pub const fn set_repository(&mut self, repository: RepositoryAccess) {
    self.repository = repository;
  }

  pub fn set_writable_folders(&mut self, folders: WritableFolders) {
    self.writable_folders = folders;
  }

  pub fn set_commands(&mut self, commands: CommandPermission) {
    self.commands = commands;
  }

  pub const fn set_network(&mut self, network: NetworkAccess) {
    self.network = network;
  }
}
