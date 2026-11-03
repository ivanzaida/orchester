//! A pull request's CI as `gh` reads it: each check run and commit status of its head with its state, and for a
//! failed GitHub Actions job the tail of its failed steps' log. Orchester only reads; how often it asks is the
//! caller's ([`POLL_INTERVAL`]), GitHub rate-limits.

pub mod fix;
mod gh;
mod message;
pub mod redact;

use std::time::Duration;

pub use self::{
  gh::{LOG_TAIL_LINES, gh_job_log_command, gh_pr_checks_command, parse_gh_job_log, parse_gh_pr_checks},
  message::fix_text,
};
use super::{SyncFailure, sync::sync_failure};
use crate::{
  connection::{GitHubCheck, GitHubState},
  github::GitHub,
  repository::{RepositoryError, RepositoryLink},
  time::Timestamp,
};

/// How long a read of a pull request's checks answers for: no read asks GitHub sooner, and while a check is
/// queued or running a screen reads again after it.
pub const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Most failed checks whose log one read asks for: each is a request of its own.
pub const MOST_LOGS: usize = 5;

/// How one check stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CiState {
  Queued,
  Running,
  Passed,
  Failed,
  Cancelled,
  /// Skipped, or superseded by a later run.
  Skipped,
}

impl CiState {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Queued => "queued",
      Self::Running => "running",
      Self::Passed => "passed",
      Self::Failed => "failed",
      Self::Cancelled => "cancelled",
      Self::Skipped => "skipped",
    }
  }

  /// Whether it may still change: queued or running.
  pub const fn is_pending(self) -> bool {
    matches!(self, Self::Queued | Self::Running)
  }
}

/// One check of a pull request's head: a GitHub Actions job, an app's check run or a commit status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CiCheck {
  pub name: String,
  /// The workflow of a GitHub Actions job.
  pub workflow: Option<String>,
  pub state: CiState,
  /// Its page, on GitHub or the service that reported it.
  pub url: Option<String>,
  /// The GitHub Actions job, whose log `gh` reads; `None` for other checks.
  pub job: Option<u64>,
  /// The end of a failed job's failed steps' log ([`LOG_TAIL_LINES`] lines), where `gh` gave it.
  pub log_tail: Option<String>,
}

/// The checks of a pull request's head, in `gh`'s order, and how they stand together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CiChecks {
  pub checks: Vec<CiCheck>,
}

/// How a pull request's checks stand together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiSummary {
  /// The head has no checks.
  None,
  /// One is queued or running, none failed yet.
  Pending,
  /// One failed or was cancelled.
  Failed,
  /// Every one passed or was skipped.
  Passed,
}

impl CiSummary {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::None => "no_checks",
      Self::Pending => "running",
      Self::Failed => "failed",
      Self::Passed => "passed",
    }
  }
}

impl CiChecks {
  /// A failure wins over a check still running: the pull request cannot pass any more.
  pub fn summary(&self) -> CiSummary {
    let any = |pick: fn(CiState) -> bool| self.checks.iter().any(|check| pick(check.state));
    if self.checks.is_empty() {
      CiSummary::None
    } else if any(|state| matches!(state, CiState::Failed | CiState::Cancelled)) {
      CiSummary::Failed
    } else if any(CiState::is_pending) {
      CiSummary::Pending
    } else {
      CiSummary::Passed
    }
  }

  /// Whether a check may still change, so a screen reads again after [`POLL_INTERVAL`].
  pub fn is_pending(&self) -> bool {
    self.checks.iter().any(|check| check.state.is_pending())
  }

  /// How many checks are in `state`.
  pub fn count(&self, state: CiState) -> usize {
    self.checks.iter().filter(|check| check.state == state).count()
  }
}

/// The check of `gh` a read made, and what it read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CiRead {
  pub check: GitHubCheck,
  pub outcome: Result<CiChecks, SyncFailure>,
}

/// Reads the checks of pull request `number` of the linked repository through `github`, if `gh` acts as the account
/// the link expects, with the log tail of at most [`MOST_LOGS`] failed GitHub Actions jobs. A log `gh` does not
/// give leaves the check without one: the check still reads.
pub fn read_ci(github: &impl GitHub, link: &RepositoryLink, number: u64, now: Timestamp) -> CiRead {
  let check = github.check(now);
  let state = GitHubState::of(Some(&check.result), Some(link.expected_login()));
  if !matches!(state, GitHubState::SignedIn { .. }) {
    return CiRead {
      check,
      outcome: Err(SyncFailure::Account(state)),
    };
  }
  let repo = link.repo();
  let mut checks = match github.pull_request_checks(repo, number) {
    Ok(checks) => checks,
    Err(error) => {
      return CiRead {
        check,
        outcome: Err(sync_failure(github, repo, error)),
      };
    }
  };
  let failed_jobs = checks
    .iter_mut()
    .filter(|check| check.state == CiState::Failed)
    .filter_map(|check| check.job.map(|job| (job, check)))
    .take(MOST_LOGS);
  for (job, check) in failed_jobs {
    match github.job_log(repo, job) {
      Ok(tail) => check.log_tail = tail,
      // A rate limit refuses the next logs too; the checks were read.
      Err(RepositoryError::RateLimited) => break,
      // GitHub keeps logs for a while only, and gives none of a job still finishing.
      Err(_) => {}
    }
  }
  CiRead {
    check,
    outcome: Ok(CiChecks { checks }),
  }
}
