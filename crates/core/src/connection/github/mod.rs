//! GitHub connections through the user's GitHub CLI (`gh`) sign-in. Orchester never signs in,
//! switches accounts or reads a token: it only asks `gh` which account is active on a host and
//! lets `gh` make the requests.

mod cli;
mod state;

use std::{fmt, ops::Deref, path::PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) use self::cli::gh_command;
pub use self::{
  cli::{
    GH_PROGRAM, GhAuthStatus, check_github, gh_auth_status_command, gh_version_command, parse_gh_auth_status,
    parse_gh_version,
  },
  state::{GhAdvice, GitHubState, REQUIRED_SCOPE},
};
use super::{Connection, ConnectionError, ConnectionKind, ConnectionStatus, LastCheck, SignIn, agent::sign_in_status};
use crate::{id::ConnectionId, name::Name, time::Timestamp};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GitHubNameError {
  #[error("`{0}` is not a valid host name")]
  Host(String),
  #[error("`{0}` is not a valid GitHub login")]
  Login(String),
}

/// A GitHub host name: `github.com` or a GitHub Enterprise Server host, lowercase, without
/// scheme, port or path.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GitHubHost(String);

impl GitHubHost {
  const MAX_BYTES: usize = 253;
  pub const GITHUB_COM: &str = "github.com";

  pub fn new(value: &str) -> Result<Self, GitHubNameError> {
    let host = value.to_ascii_lowercase();
    let valid = !host.is_empty()
      && host.len() <= Self::MAX_BYTES
      && host
        .split('.')
        .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'))
      && host
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-');
    if valid {
      Ok(Self(host))
    } else {
      Err(GitHubNameError::Host(value.to_owned()))
    }
  }

  pub fn github_com() -> Self {
    Self(Self::GITHUB_COM.to_owned())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for GitHubHost {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for GitHubHost {
  type Error = GitHubNameError;

  fn try_from(value: String) -> Result<Self, GitHubNameError> {
    Self::new(&value)
  }
}

impl From<GitHubHost> for String {
  fn from(host: GitHubHost) -> Self {
    host.0
  }
}

/// A GitHub account login. Besides user logins this admits managed-user (`name_short`) and app
/// (`name[bot]`) logins.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GitHubLogin(String);

impl GitHubLogin {
  const MAX_CHARS: usize = 100;

  pub fn new(value: &str) -> Result<Self, GitHubNameError> {
    let valid = !value.is_empty()
      && value.len() <= Self::MAX_CHARS
      && !value.starts_with('-')
      && value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '[' | ']'));
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(GitHubNameError::Login(value.to_owned()))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for GitHubLogin {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for GitHubLogin {
  type Error = GitHubNameError;

  fn try_from(value: String) -> Result<Self, GitHubNameError> {
    Self::new(&value)
  }
}

impl From<GitHubLogin> for String {
  fn from(login: GitHubLogin) -> Self {
    login.0
  }
}

/// What running `gh`'s read-only commands found for one host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum GitHubCheckResult {
  /// The program answered as the GitHub CLI.
  Available {
    version: String,
    /// The active account on the host, when `gh` names one.
    account: Option<GitHubLogin>,
    /// OAuth scopes of the active account's token; empty for tokens without scopes (fine-grained
    /// or app tokens) and when not signed in.
    scopes: Vec<String>,
    sign_in: SignIn,
  },
  /// Nothing to run at the executable path.
  ExecutableMissing,
  /// The program ran but did not answer as the GitHub CLI.
  NotRecognized,
  /// The program could not be started or did not answer in time.
  CouldNotRun,
}

impl GitHubCheckResult {
  /// The program is the GitHub CLI, so a connection to it may be saved.
  pub const fn is_verified(&self) -> bool {
    matches!(self, Self::Available { .. })
  }

  pub const fn status(&self) -> ConnectionStatus {
    match self {
      Self::Available { sign_in, .. } => sign_in_status(*sign_in),
      Self::ExecutableMissing => ConnectionStatus::ExecutableMissing,
      Self::NotRecognized => ConnectionStatus::NotRecognized,
      Self::CouldNotRun => ConnectionStatus::CouldNotRun,
    }
  }

  /// The account `gh` acts as on the host: only when it is signed in.
  pub const fn signed_in_account(&self) -> Option<&GitHubLogin> {
    match self {
      Self::Available {
        account: Some(account),
        sign_in: SignIn::SignedIn,
        ..
      } => Some(account),
      _ => None,
    }
  }
}

/// A check of the GitHub CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubCheck {
  pub at: Timestamp,
  pub result: GitHubCheckResult,
}

/// A connection to GitHub through the GitHub CLI. Derefs to the [`Connection`] it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubConnection {
  connection: Connection,
  host: GitHubHost,
}

impl GitHubConnection {
  /// A connection that has not been checked yet.
  pub fn new(
    id: ConnectionId,
    name: Name,
    host: GitHubHost,
    executable: PathBuf,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    let kind = ConnectionKind::GitHub { host: host.clone() };
    let connection = Connection::new(id, name, kind, executable, created_at)?;
    Ok(Self { connection, host })
  }

  pub const fn host(&self) -> &GitHubHost {
    &self.host
  }

  pub fn into_connection(self) -> Connection {
    self.connection
  }

  pub fn last_check(&self) -> Option<&GitHubCheck> {
    match self.connection.last_check.as_ref()? {
      LastCheck::GitHub(check) => Some(check),
      LastCheck::Agent(_) => None,
    }
  }

  pub fn record_check(&mut self, check: GitHubCheck) {
    self.connection.last_check = Some(LastCheck::GitHub(check));
  }

  pub fn rename(&mut self, name: Name) {
    self.connection.rename(name);
  }

  /// See [`Connection::set_executable`].
  pub fn set_executable(&mut self, executable: PathBuf) -> Result<(), ConnectionError> {
    self.connection.set_executable(executable)
  }

  /// What the last check means for a project that expects `expected`.
  pub fn state(&self, expected: Option<&GitHubLogin>) -> GitHubState {
    GitHubState::of(self.last_check().map(|check| &check.result), expected)
  }
}

impl TryFrom<Connection> for GitHubConnection {
  type Error = ConnectionError;

  fn try_from(connection: Connection) -> Result<Self, ConnectionError> {
    match &connection.kind {
      ConnectionKind::GitHub { host } => Ok(Self {
        host: host.clone(),
        connection,
      }),
      ConnectionKind::Agent(_) => Err(ConnectionError::NotGitHub(connection.id)),
    }
  }
}

impl Deref for GitHubConnection {
  type Target = Connection;

  fn deref(&self) -> &Connection {
    &self.connection
  }
}
