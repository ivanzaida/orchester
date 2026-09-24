//! Project connections (`docs/design/orchester-workspace.md`, "Agent profiles and connections").
//! Only agent providers exist so far: the Codex and Claude Code CLIs, run on this computer and
//! signed in through their own login. Orchester stores no credentials for them; a connection
//! holds the executable and what the last check found out about it.

mod adapter;
mod catalog;

use std::path::{Path, PathBuf};

use thiserror::Error;

pub use self::{
  adapter::{
    CommandError, CommandOutput, CommandRunner, check, executable_candidates, parse_claude_auth_status,
    parse_codex_catalog, parse_codex_login_status, parse_version,
  },
  catalog::{CatalogModel, CatalogSource, ModelCatalog, ModelSupport, REASONING_EFFORT},
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConnectionError {
  #[error("the executable path `{0}` is not absolute")]
  RelativeExecutable(PathBuf),
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

/// What running the CLI's read-only commands found.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CheckResult {
  /// The program answered as the expected CLI.
  Available {
    version: String,
    sign_in: SignIn,
    catalog: ModelCatalog,
  },
  /// Nothing to run at the executable path.
  ExecutableMissing,
  /// The program ran but did not answer as the expected CLI.
  NotRecognized,
  /// The program could not be started or did not answer in time.
  CouldNotRun,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionCheck {
  pub at: Timestamp,
  pub result: CheckResult,
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

  /// Agents can be started through the connection, or at least nothing says they cannot.
  pub const fn is_usable(self) -> bool {
    matches!(self, Self::Connected | Self::SignInUnknown)
  }
}

/// A project connection to an agent provider CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
  id: ConnectionId,
  name: Name,
  provider: AgentProvider,
  executable: PathBuf,
  last_check: Option<ConnectionCheck>,
  created_at: Timestamp,
}

impl Connection {
  /// A connection that has not been checked yet.
  pub fn new(
    id: ConnectionId,
    name: Name,
    provider: AgentProvider,
    executable: PathBuf,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    Ok(Self {
      id,
      name,
      provider,
      executable: absolute(executable)?,
      last_check: None,
      created_at,
    })
  }

  pub fn from_stored(
    id: ConnectionId,
    name: Name,
    provider: AgentProvider,
    executable: PathBuf,
    last_check: Option<ConnectionCheck>,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    let mut connection = Self::new(id, name, provider, executable, created_at)?;
    connection.last_check = last_check;
    Ok(connection)
  }

  pub const fn id(&self) -> ConnectionId {
    self.id
  }

  pub fn name(&self) -> &Name {
    &self.name
  }

  pub const fn provider(&self) -> AgentProvider {
    self.provider
  }

  pub fn executable(&self) -> &Path {
    &self.executable
  }

  pub const fn last_check(&self) -> Option<&ConnectionCheck> {
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

  pub fn record_check(&mut self, check: ConnectionCheck) {
    self.last_check = Some(check);
  }

  pub fn status(&self) -> ConnectionStatus {
    let Some(check) = &self.last_check else {
      return ConnectionStatus::NotChecked;
    };
    match &check.result {
      CheckResult::Available { sign_in, .. } => match sign_in {
        SignIn::SignedIn => ConnectionStatus::Connected,
        SignIn::SignedOut => ConnectionStatus::SignInRequired,
        SignIn::Unknown => ConnectionStatus::SignInUnknown,
      },
      CheckResult::ExecutableMissing => ConnectionStatus::ExecutableMissing,
      CheckResult::NotRecognized => ConnectionStatus::NotRecognized,
      CheckResult::CouldNotRun => ConnectionStatus::CouldNotRun,
    }
  }

  /// CLI version found by the last successful check.
  pub fn version(&self) -> Option<&str> {
    match &self.last_check.as_ref()?.result {
      CheckResult::Available { version, .. } => Some(version),
      _ => None,
    }
  }

  /// Models found by the last successful check.
  pub fn catalog(&self) -> Option<&ModelCatalog> {
    match &self.last_check.as_ref()?.result {
      CheckResult::Available { catalog, .. } => Some(catalog),
      _ => None,
    }
  }

  /// Models a profile can choose from on this connection, in catalog order.
  pub fn models(&self) -> &[CatalogModel] {
    self.catalog().map_or(&[], ModelCatalog::models)
  }
}

fn absolute(path: PathBuf) -> Result<PathBuf, ConnectionError> {
  if path.is_absolute() {
    Ok(path)
  } else {
    Err(ConnectionError::RelativeExecutable(path))
  }
}
