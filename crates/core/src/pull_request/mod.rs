//! Pull requests of the project's linked GitHub repository, as the signed-in account sees them
//! (`docs/design/orchester-workspace.md`, "Pull requests" and "GitHub"). Orchester only reads
//! them; a pull request's GitHub state is kept apart from agent readiness, which comes with runs.

mod checks;
mod detail;
pub mod diff;
mod gh;
mod sync;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  checks::{gh_check_rollups_command, parse_gh_check_rollups},
  detail::{
    DetailRefresh, DiffRead, MAX_DIFF_FILES, MAX_DIFF_LINES, PullRequestHead, RefreshedDetail, RevisionDiff,
    gh_compare_diff_command, gh_pr_view_command, is_too_large, parse_gh_compare_diff, parse_gh_pr_view,
    refresh_pull_request,
  },
  gh::{PR_LIST_FIELDS, gh_pr_list_command, gh_rate_limit_command, parse_gh_pr_list, parse_gh_rate_limit},
  sync::{SYNC_LIMIT, SyncFailure, SyncResult, sync_pull_requests},
};
use crate::{repository::GitHubRepo, revision::Revision, run::BranchName, time::Timestamp};

/// Why a pull request as read from GitHub or from storage is not accepted.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PullRequestError {
  #[error("{0} is not a pull request number")]
  Number(u64),
  #[error("a pull request has a title")]
  EmptyTitle,
  #[error("`{0}` is not the address of a pull request on the repository's host")]
  Url(String),
  #[error("`{0}` is not a GitHub login")]
  Author(String),
  #[error("the checks are not those of the pull request's head revision")]
  ChecksNotOnHead,
}

/// A pull request's state on GitHub. A draft is open but not ready for review yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PullRequestState {
  Open,
  Draft,
  Merged,
  Closed,
}

impl PullRequestState {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Open => "open",
      Self::Draft => "draft",
      Self::Merged => "merged",
      Self::Closed => "closed",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    [Self::Open, Self::Draft, Self::Merged, Self::Closed]
      .into_iter()
      .find(|state| state.as_str() == value)
  }
}

/// What reviews on GitHub decided (GitHub's `reviewDecision`); none when the repository does
/// not require reviews.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReviewDecision {
  ReviewRequired,
  Approved,
  ChangesRequested,
}

impl ReviewDecision {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::ReviewRequired => "review_required",
      Self::Approved => "approved",
      Self::ChangesRequested => "changes_requested",
    }
  }

  /// Reads [`Self::as_str`] and GitHub's upper-case spelling.
  pub fn parse(value: &str) -> Option<Self> {
    [Self::ReviewRequired, Self::Approved, Self::ChangesRequested]
      .into_iter()
      .find(|decision| decision.as_str().eq_ignore_ascii_case(value))
  }
}

/// Size of a pull request's change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
  pub files: u32,
  pub additions: u32,
  pub deletions: u32,
}

/// GitHub's checks (check runs and commit statuses) of one revision, by outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckRollup {
  pub passed: u32,
  pub failed: u32,
  /// Queued or still running.
  pub pending: u32,
  pub skipped: u32,
  /// The revision the checks ran on.
  pub on: Revision,
}

impl CheckRollup {
  /// No checks on `on` yet.
  pub const fn none(on: Revision) -> Self {
    Self {
      passed: 0,
      failed: 0,
      pending: 0,
      skipped: 0,
      on,
    }
  }

  pub const fn total(&self) -> u32 {
    self.passed + self.failed + self.pending + self.skipped
  }
}

/// Everything a pull request is made of, checked by [`PullRequest::new`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequestParts {
  pub number: u64,
  pub title: String,
  pub url: String,
  pub state: PullRequestState,
  pub head: BranchName,
  pub base: BranchName,
  pub head_sha: Revision,
  /// The author's login; none for a deleted account.
  pub author: Option<String>,
  pub changes: Changes,
  pub checks: CheckRollup,
  pub review_decision: Option<ReviewDecision>,
  pub updated_at: Timestamp,
}

/// A pull request of a repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequest {
  parts: PullRequestParts,
}

impl PullRequest {
  /// A pull request of `repo`: a positive number, a title, its web address on the repository's
  /// host (without credentials), and checks of its head revision.
  pub fn new(repo: &GitHubRepo, parts: PullRequestParts) -> Result<Self, PullRequestError> {
    if parts.number == 0 {
      return Err(PullRequestError::Number(parts.number));
    }
    if parts.title.trim().is_empty() {
      return Err(PullRequestError::EmptyTitle);
    }
    if !is_pull_request_url(repo, parts.number, &parts.url) {
      return Err(PullRequestError::Url(parts.url));
    }
    if let Some(author) = &parts.author {
      // GitHub App authors read `app/<name>`.
      let valid = !author.is_empty()
        && author
          .chars()
          .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/' | '[' | ']'));
      if !valid {
        return Err(PullRequestError::Author(author.clone()));
      }
    }
    if parts.checks.on != parts.head_sha {
      return Err(PullRequestError::ChecksNotOnHead);
    }
    Ok(Self { parts })
  }

  pub const fn number(&self) -> u64 {
    self.parts.number
  }

  pub fn title(&self) -> &str {
    &self.parts.title
  }

  /// The pull request on GitHub's website.
  pub fn url(&self) -> &str {
    &self.parts.url
  }

  pub const fn state(&self) -> PullRequestState {
    self.parts.state
  }

  pub const fn head(&self) -> &BranchName {
    &self.parts.head
  }

  pub const fn base(&self) -> &BranchName {
    &self.parts.base
  }

  pub const fn head_sha(&self) -> &Revision {
    &self.parts.head_sha
  }

  pub fn author(&self) -> Option<&str> {
    self.parts.author.as_deref()
  }

  pub const fn changes(&self) -> Changes {
    self.parts.changes
  }

  pub const fn checks(&self) -> &CheckRollup {
    &self.parts.checks
  }

  /// The pull request with `checks`, which must be of its head revision.
  pub fn with_checks(mut self, checks: CheckRollup) -> Result<Self, PullRequestError> {
    if checks.on != self.parts.head_sha {
      return Err(PullRequestError::ChecksNotOnHead);
    }
    self.parts.checks = checks;
    Ok(self)
  }

  pub const fn review_decision(&self) -> Option<ReviewDecision> {
    self.parts.review_decision
  }

  pub const fn updated_at(&self) -> Timestamp {
    self.parts.updated_at
  }

  /// Whether a search for `query` finds the pull request: by its number (`42`, `#42`), a part
  /// of its title or of its head or base branch, ignoring case. An empty query finds all.
  pub fn matches(&self, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
      return true;
    }
    let number = self.parts.number.to_string();
    query.strip_prefix('#').unwrap_or(&query) == number
      || self.parts.title.to_lowercase().contains(&query)
      || self.parts.head.as_str().to_lowercase().contains(&query)
      || self.parts.base.as_str().to_lowercase().contains(&query)
  }
}

/// The pull requests of a repository as one sync read them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncedPullRequests {
  pub synced_at: Timestamp,
  /// Most recently updated first.
  pub pull_requests: Vec<PullRequest>,
}

/// Most recently updated first; the higher number first among equals.
pub fn sort_by_update(pull_requests: &mut [PullRequest]) {
  pull_requests.sort_by(|a, b| {
    b.updated_at()
      .cmp(&a.updated_at())
      .then_with(|| b.number().cmp(&a.number()))
  });
}

/// `https://<host>/<owner>/<name>/pull/<number>`. Owner and name are not compared: GitHub
/// reports addresses under the repository's current name after a rename.
fn is_pull_request_url(repo: &GitHubRepo, number: u64, url: &str) -> bool {
  let Some(rest) = url.strip_prefix("https://") else {
    return false;
  };
  let segments: Vec<&str> = rest.split('/').collect();
  let number = number.to_string();
  matches!(
    segments.as_slice(),
    [host, owner, name, "pull", found]
      if *host == repo.host().as_str() && is_path_segment(owner) && is_path_segment(name) && *found == number
  )
}

fn is_path_segment(segment: &str) -> bool {
  !segment.is_empty()
    && segment
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
