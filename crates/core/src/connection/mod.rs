//! Project connections (`docs/design/orchester-workspace.md`, "Agent profiles and connections").
//! Two kinds exist: agent providers (the Codex and Claude Code CLIs) and GitHub through the
//! GitHub CLI. Each runs on this computer and is signed in through the program's own login;
//! Orchester stores no credentials and never asks the programs for them. A connection holds the
//! executable and what the last check found out about it.

mod adapter;
mod agent;
mod catalog;
mod github;

use std::path::{Path, PathBuf};

use thiserror::Error;

pub(crate) use self::github::gh_command;
pub use self::{
  adapter::{
    check, executable_candidates, parse_claude_auth_status, parse_codex_catalog, parse_codex_login_status,
    parse_version,
  },
  agent::{AgentConnection, CheckResult, ConnectionCheck},
  catalog::{CatalogModel, CatalogSource, ModelCatalog, ModelSupport, REASONING_EFFORT},
  github::{
    GH_PROGRAM, GhAdvice, GhAuthStatus, GitHubCheck, GitHubCheckResult, GitHubConnection, GitHubHost, GitHubLogin,
    GitHubNameError, GitHubState, REQUIRED_SCOPE, check_github, gh_auth_status_command, gh_version_command,
    parse_gh_auth_status, parse_gh_version,
  },
};
use crate::{id::ConnectionId, name::Name, time::Timestamp};

/// The agent CLIs Orchester can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentProvider {
  CodexCli,
  ClaudeCodeCli,
}

impl AgentProvider {
  pub const ALL: [Self; 2] = [Self::CodexCli, Self::ClaudeCodeCli];

  /// Name of the program on the `PATH`.
  pub const fn program_name(self) -> &'static str {
    match self {
      Self::CodexCli => "codex",
      Self::ClaudeCodeCli => "claude",
    }
  }
}

/// What a connection connects to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionKind {
  /// An agent CLI that runs the steps of agent profiles.
  Agent(AgentProvider),
  /// The GitHub CLI, signed in on `host`.
  GitHub { host: GitHubHost },
}

impl ConnectionKind {
  /// Name of the program on the `PATH`.
  pub const fn program_name(&self) -> &'static str {
    match self {
      Self::Agent(provider) => provider.program_name(),
      Self::GitHub { .. } => GH_PROGRAM,
    }
  }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConnectionError {
  #[error("the executable path `{0}` is not absolute")]
  RelativeExecutable(PathBuf),
  #[error("connection {0} is not an agent provider connection")]
  NotAgent(ConnectionId),
  #[error("connection {0} is not a GitHub connection")]
  NotGitHub(ConnectionId),
  #[error("the check does not belong to the kind of connection {0}")]
  CheckKindMismatch(ConnectionId),
}

/// Whether the CLI is signed in, as far as its own status command tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignIn {
  SignedIn,
  SignedOut,
  /// The CLI has no status command, or its answer could not be read.
  Unknown,
}

/// State of a connection, derived from its last check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
  NotChecked,
  Connected,
  SignInRequired,
  /// The CLI works, but whether it is signed in could not be determined.
  SignInUnknown,
  ExecutableMissing,
  NotRecognized,
  CouldNotRun,
}

impl ConnectionStatus {
  /// Stored alongside the check for fast reads.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::NotChecked => "not_checked",
      Self::Connected => "connected",
      Self::SignInRequired => "sign_in_required",
      Self::SignInUnknown => "sign_in_unknown",
      Self::ExecutableMissing => "executable_missing",
      Self::NotRecognized => "not_recognized",
      Self::CouldNotRun => "could_not_run",
    }
  }

  /// The program can be used through the connection, or at least nothing says it cannot.
  pub const fn is_usable(self) -> bool {
    matches!(self, Self::Connected | Self::SignInUnknown)
  }
}

/// The last check of a connection, of the connection's kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LastCheck {
  Agent(ConnectionCheck),
  GitHub(GitHubCheck),
}

impl LastCheck {
  pub const fn at(&self) -> Timestamp {
    match self {
      Self::Agent(check) => check.at,
      Self::GitHub(check) => check.at,
    }
  }

  pub const fn status(&self) -> ConnectionStatus {
    match self {
      Self::Agent(check) => check.result.status(),
      Self::GitHub(check) => check.result.status(),
    }
  }

  /// Version of the program, when the check found the expected one.
  pub fn version(&self) -> Option<&str> {
    match self {
      Self::Agent(ConnectionCheck {
        result: CheckResult::Available { version, .. },
        ..
      })
      | Self::GitHub(GitHubCheck {
        result: GitHubCheckResult::Available { version, .. },
        ..
      }) => Some(version),
      Self::Agent(_) | Self::GitHub(_) => None,
    }
  }

  const fn fits(&self, kind: &ConnectionKind) -> bool {
    matches!(
      (self, kind),
      (Self::Agent(_), ConnectionKind::Agent(_)) | (Self::GitHub(_), ConnectionKind::GitHub { .. })
    )
  }
}

/// A project connection. [`AgentConnection`] and [`GitHubConnection`] are the typed views that
/// code needing one kind works with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
  id: ConnectionId,
  name: Name,
  kind: ConnectionKind,
  executable: PathBuf,
  last_check: Option<LastCheck>,
  created_at: Timestamp,
}

impl Connection {
  /// A connection that has not been checked yet.
  pub fn new(
    id: ConnectionId,
    name: Name,
    kind: ConnectionKind,
    executable: PathBuf,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    Ok(Self {
      id,
      name,
      kind,
      executable: absolute(executable)?,
      last_check: None,
      created_at,
    })
  }

  pub fn from_stored(
    id: ConnectionId,
    name: Name,
    kind: ConnectionKind,
    executable: PathBuf,
    last_check: Option<LastCheck>,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    let mut connection = Self::new(id, name, kind, executable, created_at)?;
    if let Some(check) = last_check {
      connection.record_check(check)?;
    }
    Ok(connection)
  }

  pub const fn id(&self) -> ConnectionId {
    self.id
  }

  pub fn name(&self) -> &Name {
    &self.name
  }

  pub const fn kind(&self) -> &ConnectionKind {
    &self.kind
  }

  pub fn executable(&self) -> &Path {
    &self.executable
  }

  pub const fn last_check(&self) -> Option<&LastCheck> {
    self.last_check.as_ref()
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  pub fn rename(&mut self, name: Name) {
    self.name = name;
  }

  /// Points the connection at another program. The last check described the previous one,
  /// so it is dropped.
  pub fn set_executable(&mut self, executable: PathBuf) -> Result<(), ConnectionError> {
    let executable = absolute(executable)?;
    if executable != self.executable {
      self.executable = executable;
      self.last_check = None;
    }
    Ok(())
  }

  /// Replaces the last check. A check of another kind of connection is refused.
  pub fn record_check(&mut self, check: LastCheck) -> Result<(), ConnectionError> {
    if !check.fits(&self.kind) {
      return Err(ConnectionError::CheckKindMismatch(self.id));
    }
    self.last_check = Some(check);
    Ok(())
  }

  pub fn status(&self) -> ConnectionStatus {
    self
      .last_check
      .as_ref()
      .map_or(ConnectionStatus::NotChecked, LastCheck::status)
  }

  /// Program version found by the last successful check.
  pub fn version(&self) -> Option<&str> {
    self.last_check.as_ref()?.version()
  }
}

fn absolute(path: PathBuf) -> Result<PathBuf, ConnectionError> {
  if path.is_absolute() {
    Ok(path)
  } else {
    Err(ConnectionError::RelativeExecutable(path))
  }
}
