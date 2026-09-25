//! The project's GitHub repository. The local folder is the repository; its GitHub counterpart
//! is found from a git remote (`origin` unless the user picks another) and confirmed through a
//! GitHub connection. Linking is optional and a project links at most one repository. The link
//! records the account that confirmed it, so work is not published under another account that
//! `gh` happens to have active later.

mod detect;
mod link;
mod remote;
mod url;
mod view;

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) use self::view::{classify_failure, unrecognized};
pub use self::{
  detect::{DetectRequest, Detection, DetectionState, GhProgram, GitHubDetection, detect_repository},
  link::link_repository,
  remote::{Remote, git_remotes_command, parse_git_remotes, read_remotes},
  url::{GitHubRepo, RepoNameError, redact_userinfo, strip_userinfo},
  view::{RepositoryView, gh_repo_view_command, parse_gh_repo_view},
};
use crate::{
  command::CommandError,
  connection::{ConnectionStatus, GitHubHost, GitHubLogin},
  id::ConnectionId,
  run::BranchName,
  time::Timestamp,
};

#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not a valid git remote name")]
pub struct RemoteNameError(String);

/// The name of a git remote, e.g. `origin`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RemoteName(String);

impl RemoteName {
  const MAX_BYTES: usize = 255;
  /// The remote `git clone` creates, looked up unless the user picks another.
  pub const ORIGIN: &str = "origin";

  pub fn new(value: &str) -> Result<Self, RemoteNameError> {
    let valid = !value.is_empty()
      && value.len() <= Self::MAX_BYTES
      && !value.starts_with('-')
      && !value.chars().any(|c| c.is_whitespace() || c.is_control());
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(RemoteNameError(value.to_owned()))
    }
  }

  pub fn origin() -> Self {
    Self(Self::ORIGIN.to_owned())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for RemoteName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for RemoteName {
  type Error = RemoteNameError;

  fn try_from(value: String) -> Result<Self, RemoteNameError> {
    Self::new(&value)
  }
}

impl From<RemoteName> for String {
  fn from(name: RemoteName) -> Self {
    name.0
  }
}

/// What the signed-in account may do in the repository (GitHub's `viewerPermission`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RepoPermission {
  Admin,
  Maintain,
  Write,
  Triage,
  Read,
}

impl RepoPermission {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Admin => "admin",
      Self::Maintain => "maintain",
      Self::Write => "write",
      Self::Triage => "triage",
      Self::Read => "read",
    }
  }

  /// Reads [`Self::as_str`] and GitHub's upper-case spelling.
  pub fn parse(value: &str) -> Option<Self> {
    [Self::Admin, Self::Maintain, Self::Write, Self::Triage, Self::Read]
      .into_iter()
      .find(|permission| permission.as_str().eq_ignore_ascii_case(value))
  }

  /// Branches can be pushed and pull requests opened from them.
  pub const fn can_push(self) -> bool {
    matches!(self, Self::Admin | Self::Maintain | Self::Write)
  }
}

/// The project's link to its GitHub repository, as confirmed at `checked_at`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryLink {
  connection: ConnectionId,
  remote: RemoteName,
  view: RepositoryView,
  expected_login: GitHubLogin,
  checked_at: Timestamp,
}

impl RepositoryLink {
  /// `view` is the repository as `expected_login` saw it at `checked_at`.
  pub const fn new(
    connection: ConnectionId,
    remote: RemoteName,
    view: RepositoryView,
    expected_login: GitHubLogin,
    checked_at: Timestamp,
  ) -> Self {
    Self {
      connection,
      remote,
      view,
      expected_login,
      checked_at,
    }
  }

  /// The GitHub connection the repository is reached through.
  pub const fn connection(&self) -> ConnectionId {
    self.connection
  }

  pub const fn remote(&self) -> &RemoteName {
    &self.remote
  }

  pub const fn repo(&self) -> &GitHubRepo {
    &self.view.repo
  }

  pub const fn default_branch(&self) -> &BranchName {
    &self.view.default_branch
  }

  pub const fn permission(&self) -> RepoPermission {
    self.view.permission
  }

  /// The account that confirmed the link.
  pub const fn expected_login(&self) -> &GitHubLogin {
    &self.expected_login
  }

  pub const fn checked_at(&self) -> Timestamp {
    self.checked_at
  }

  /// Makes the project expect `active`, the account `gh` acts as now (from a fresh check). This
  /// records the account only; `gh` itself is never changed.
  pub fn expect_account(&mut self, active: Option<&GitHubLogin>) -> Result<(), RepositoryError> {
    let active = active.ok_or_else(|| RepositoryError::NotSignedIn(self.view.repo.host().clone()))?;
    self.expected_login = active.clone();
    Ok(())
  }

  /// Fails unless `active`, the account `gh` currently acts as, is the one that confirmed the
  /// link. Anything published to GitHub for the project is checked against this first.
  pub fn ensure_account(&self, active: Option<&GitHubLogin>) -> Result<(), RepositoryError> {
    match active {
      None => Err(RepositoryError::NotSignedIn(self.view.repo.host().clone())),
      Some(active) if active != &self.expected_login => Err(RepositoryError::AccountMismatch {
        expected: self.expected_login.clone(),
        active: active.clone(),
      }),
      Some(_) => Ok(()),
    }
  }
}

/// Why the repository could not be linked or read. Program output in these errors has had
/// URL credentials redacted.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RepositoryError {
  #[error("the folder is not a git repository")]
  NotAGitRepository,
  #[error("git failed: {0}")]
  Git(String),
  #[error("the repository has no remote named `{0}`")]
  NoGitHubRemote(RemoteName),
  #[error("remote `{remote}` ({url}) is not a repository on {host}")]
  NonGitHubRemote {
    remote: RemoteName,
    /// The remote URL without its user part.
    url: String,
    host: GitHubHost,
  },
  #[error("{repo} is not on {host}, the host of the connection")]
  OtherHost { repo: GitHubRepo, host: GitHubHost },
  #[error("the GitHub CLI cannot be used ({0:?})")]
  GitHubUnavailable(ConnectionStatus),
  #[error("the GitHub CLI is not signed in on {0}")]
  NotSignedIn(GitHubHost),
  #[error("the GitHub CLI acts as {active}, but the project was linked by {expected}")]
  AccountMismatch { expected: GitHubLogin, active: GitHubLogin },
  #[error("{0} does not exist or the signed-in account cannot see it")]
  NotFound(GitHubRepo),
  #[error("the signed-in account has no access to {0}")]
  NoAccess(GitHubRepo),
  #[error("{repo} has no pull request #{number} the signed-in account can see")]
  PullRequestNotFound { repo: GitHubRepo, number: u64 },
  #[error("{0} has no diff between the pull request's revisions")]
  DiffNotFound(GitHubRepo),
  #[error("{0} has no default branch yet")]
  NoDefaultBranch(GitHubRepo),
  #[error("the GitHub API rate limit is exhausted")]
  RateLimited,
  #[error("cannot reach {host}: {detail}")]
  Network { host: GitHubHost, detail: String },
  /// GitHub was reached but failed to answer: a server error (HTTP 5xx, `status` when `gh`
  /// named it), a request it gave up on, or an answer cut off.
  #[error("{host} failed to answer (HTTP {status:?}): {detail}")]
  ServerError {
    host: GitHubHost,
    status: Option<u16>,
    detail: String,
  },
  #[error("unexpected answer from {program}: {detail}")]
  Unrecognized { program: &'static str, detail: String },
  #[error("cannot run {program}: {source}")]
  Run {
    program: &'static str,
    source: CommandError,
  },
}
