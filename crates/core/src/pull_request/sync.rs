//! One sync of the project's pull requests: the account `gh` acts as must be the one the
//! project expects (`docs/design/orchester-workspace.md`, "GitHub", rule 6), then the open pull
//! requests are read, and with them, for the Pull requests page's Merged and Closed tabs, the most
//! recently updated closed ones. A rate limit is reported with the time it resets.

use super::{Fields, ListedStates, PullRequest};
use crate::{
  connection::{GitHubCheck, GitHubState},
  github::GitHub,
  repository::{GitHubRepo, RepositoryError, RepositoryLink},
  time::Timestamp,
};

/// Most open pull requests one sync reads, and most closed ones: 100 in one request makes GitHub
/// time out on large repositories (microsoft/vscode, sharkdp/bat, BurntSushi/ripgrep), 50 answer in
/// a few seconds.
pub const SYNC_LIMIT: u32 = 50;

/// Which pull requests a sync reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncScope {
  /// The open ones (the deprecated desktop's inbox).
  Open,
  /// The open ones and the most recently updated closed or merged ones.
  OpenAndClosed,
}

impl SyncScope {
  /// What the deprecated desktop's inbox asks for, or the page's fields.
  pub const fn fields(self) -> Fields {
    match self {
      Self::Open => Fields::Inbox,
      Self::OpenAndClosed => Fields::Page,
    }
  }
}

/// Why a sync did not read the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncFailure {
  /// `gh` cannot act as the expected account; the state says why (not signed in, another
  /// account, a missing scope, no program).
  Account(GitHubState),
  /// GitHub refuses requests until `resets_at`, when `gh` could tell.
  RateLimited { resets_at: Option<Timestamp> },
  /// GitHub was not reached or did not give the list.
  Repository(RepositoryError),
}

impl SyncFailure {
  /// Until when syncing waits, automatically or on Refresh: a rate limit until it resets. An
  /// unknown reset does not hold the next sync back.
  pub const fn paused_until(&self) -> Option<Timestamp> {
    match self {
      Self::RateLimited { resets_at } => *resets_at,
      Self::Account(_) | Self::Repository(_) => None,
    }
  }

  /// Whether a sync at `now` would still be refused.
  pub fn is_paused(&self, now: Timestamp) -> bool {
    self.paused_until().is_some_and(|until| now < until)
  }
}

/// The check of `gh` the sync made, and what it read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncResult {
  pub check: GitHubCheck,
  /// The pull requests of the scope, most recently updated first.
  pub outcome: Result<Vec<PullRequest>, SyncFailure>,
}

/// Reads the pull requests of `scope` of the linked repository through `github`, if `gh` acts as
/// the account the link expects. Either list failing fails the sync.
pub fn sync_pull_requests(github: &impl GitHub, link: &RepositoryLink, scope: SyncScope, now: Timestamp) -> SyncResult {
  let check = github.check(now);
  let state = GitHubState::of(Some(&check.result), Some(link.expected_login()));
  if !matches!(state, GitHubState::SignedIn { .. }) {
    return SyncResult {
      check,
      outcome: Err(SyncFailure::Account(state)),
    };
  }
  let outcome = match read(github, link.repo(), scope) {
    Ok(mut pull_requests) => {
      super::sort_by_update(&mut pull_requests);
      Ok(pull_requests)
    }
    Err(error) => Err(sync_failure(github, link.repo(), error)),
  };
  SyncResult { check, outcome }
}

/// The open pull requests, then the closed ones. One closed between the two requests is in both lists;
/// the closed list, read later, says how it stands.
fn read(github: &impl GitHub, repo: &GitHubRepo, scope: SyncScope) -> Result<Vec<PullRequest>, RepositoryError> {
  let fields = scope.fields();
  let mut pull_requests = github.list_pull_requests(repo, (ListedStates::Open, fields), SYNC_LIMIT)?;
  if scope == SyncScope::OpenAndClosed {
    let closed = github.list_pull_requests(repo, (ListedStates::Closed, fields), SYNC_LIMIT)?;
    pull_requests.retain(|open| closed.iter().all(|closed| closed.number() != open.number()));
    pull_requests.extend(closed);
  }
  Ok(pull_requests)
}

/// How the latest sync of a repository went: when it ran, and why it did not read the list, if it did not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncAttempt {
  pub at: Timestamp,
  pub failure: Option<SyncFailure>,
}

impl SyncAttempt {
  /// Whether a sync at `now` waits: a rate limit has not reset yet.
  pub fn is_paused(&self, now: Timestamp) -> bool {
    self.failure.as_ref().is_some_and(|failure| failure.is_paused(now))
  }
}

/// What `error`, from a request about `repo`, means for syncing; a rate limit asks `github` when
/// it resets.
pub(super) fn sync_failure(github: &impl GitHub, repo: &GitHubRepo, error: RepositoryError) -> SyncFailure {
  match error {
    RepositoryError::RateLimited => SyncFailure::RateLimited {
      // Without the reset time the screen still says it is rate limited.
      resets_at: github.rate_limit_reset(repo).ok(),
    },
    error => SyncFailure::Repository(error),
  }
}
