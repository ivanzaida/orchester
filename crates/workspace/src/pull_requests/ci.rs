//! A pull request's CI on this computer, for screens and the domain tools: the checks of its head as `gh` reads
//! them now ([`orchester_core::pull_request::ci`]), the failed ones with the tail of their log, or why they were not
//! read — no linked repository, no pull request, no `gh`, no sign-in — never a pass Orchester did not read. A read
//! answers for [`POLL_INTERVAL`]: [`CiReads`] keeps it, so a screen or a client asking again sooner gets it instead
//! of asking GitHub, and [`PullRequestCi::next_read_at`] says when a screen reads again while a check still runs.
//! Every read blocks on SQLite and `gh`; the UI calls it off the UI thread.

use std::{collections::HashMap, path::Path, sync::Arc};

use orchester_core::{
  command::CommandError,
  connection::GitHubState,
  github::GhCli,
  id::ProjectId,
  pull_request::{
    PullRequest, PullRequestState, SyncFailure,
    ci::{CiChecks, POLL_INTERVAL, read_ci},
  },
  repository::RepositoryError,
  run::Run,
  store::{ConnectionStore, PullRequestStore, RepositoryStore, RunStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use parking_lot::Mutex;

use super::{InboxError, detail::hold_if_rate_limited, github_connection};

/// What is known of a pull request's CI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PullRequestCi {
  /// The project has no linked GitHub repository.
  NotLinked,
  /// The run's branch has no pull request the last sync read, or GitHub has no pull request of that number.
  NoPullRequest,
  /// The link's `gh` program is not there.
  GhMissing,
  /// `gh` is not signed in on the repository's host.
  NotSignedIn,
  /// `gh` cannot act as the account the project expects for another reason: another account, a missing scope, a
  /// program that does not answer as the GitHub CLI.
  Unavailable(GitHubState),
  /// GitHub refuses requests until `resets_at`, when `gh` could tell.
  RateLimited {
    resets_at: Option<Timestamp>,
  },
  /// GitHub was asked and did not give the checks.
  NotRead(RepositoryError),
  Read(ReadCi),
}

/// The checks of pull request `number` as one read gave them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadCi {
  pub number: u64,
  pub checks: CiChecks,
  pub read_at: Timestamp,
}

impl PullRequestCi {
  /// When a screen reads the checks again: [`POLL_INTERVAL`] after a read that found one queued or running;
  /// `None` when nothing will change without a new push, or nothing was read.
  pub fn next_read_at(&self) -> Option<Timestamp> {
    match self {
      Self::Read(read) if read.checks.is_pending() => Some(after_poll(read.read_at)),
      _ => None,
    }
  }

  /// What a read that failed with `failure` came to.
  fn after(failure: SyncFailure) -> Self {
    match failure {
      SyncFailure::Account(GitHubState::ProgramMissing)
      | SyncFailure::Repository(RepositoryError::Run {
        source: CommandError::NotFound,
        ..
      }) => Self::GhMissing,
      SyncFailure::Account(GitHubState::NotSignedIn) | SyncFailure::Repository(RepositoryError::NotSignedIn(_)) => {
        Self::NotSignedIn
      }
      SyncFailure::Account(state) => Self::Unavailable(state),
      SyncFailure::RateLimited { resets_at } => Self::RateLimited { resets_at },
      SyncFailure::Repository(RepositoryError::PullRequestNotFound { .. }) => Self::NoPullRequest,
      SyncFailure::Repository(error) => Self::NotRead(error),
    }
  }
}

fn after_poll(at: Timestamp) -> Timestamp {
  let interval = i64::try_from(POLL_INTERVAL.as_millis()).expect("the poll interval is 30 s");
  Timestamp::from_unix_millis(at.unix_millis() + interval)
}

/// Reads the checks of pull request `number` from GitHub with `programs` at `now`, recording the check of `gh` it
/// made on the link's connection. While a rate limit the pull requests' sync recorded has not reset, nothing asks
/// GitHub; a rate limit this read meets holds the syncs too. Screens and clients read through [`CiReads`].
pub fn read_ci_in(
  repo_root: &Path,
  programs: &Programs,
  number: u64,
  now: Timestamp,
) -> Result<PullRequestCi, InboxError> {
  let mut db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(PullRequestCi::NotLinked);
  };
  if let Some(attempt) = db.pull_request_sync(link.repo())?
    && let Some(failure) = attempt.failure.filter(|failure| failure.is_paused(now))
  {
    return Ok(PullRequestCi::after(failure));
  }
  let mut connection = github_connection(&db, link.connection())?;
  let read = read_ci(&GhCli::new(programs.runner(), &connection), &link, number, now);
  connection.record_check(read.check);
  db.save_connection(&connection)?;
  match read.outcome {
    Ok(checks) => Ok(PullRequestCi::Read(ReadCi {
      number,
      checks,
      read_at: now,
    })),
    Err(failure) => {
      tracing::info!(repository = %link.repo(), number, ?failure, "the checks of the pull request were not read");
      hold_if_rate_limited(&mut db, link.repo(), &failure, now)?;
      Ok(PullRequestCi::after(failure))
    }
  }
}

/// The number of the pull request of `run`'s branch the last sync stored ([`stored_run_pull_request`]).
pub fn run_pull_request(db: &ProjectDb, run: &Run) -> Result<Option<u64>, InboxError> {
  Ok(stored_run_pull_request(db, run)?.map(|pull_request| pull_request.number()))
}

/// The pull request of `run`'s branch the last sync stored: its open one, else the most recently updated one.
/// `None` without a link or such a pull request.
pub fn stored_run_pull_request(db: &ProjectDb, run: &Run) -> Result<Option<PullRequest>, InboxError> {
  let Some(link) = db.repository_link()? else {
    return Ok(None);
  };
  let Some(synced) = db.pull_requests(link.repo())? else {
    return Ok(None);
  };
  // A fork's branch of the same name is not the run's.
  let mut of_run: Vec<PullRequest> = synced
    .pull_requests
    .into_iter()
    .filter(|pull_request| pull_request.head() == run.branch() && !pull_request.is_cross_repository())
    .collect();
  let open = of_run
    .iter()
    .position(|pull_request| matches!(pull_request.state(), PullRequestState::Open | PullRequestState::Draft));
  Ok(match open {
    Some(at) => Some(of_run.swap_remove(at)),
    None => of_run.into_iter().next(),
  })
}

/// A run's pull request as the last sync stored it, with its CI as [`CiReads`] gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunCi {
  pub pull_request: PullRequest,
  pub ci: PullRequestCi,
}

/// The pull request of run `run` of `project`, at `root`, with its CI read through `reads` at `now`; `None` while the
/// run's branch has no pull request the last sync stored. Blocks on SQLite and `gh`.
pub fn run_ci_in(
  reads: &CiReads,
  (project, root): (ProjectId, &Path),
  programs: &Programs,
  (run, now): (orchester_core::id::RunId, Timestamp),
) -> Result<Option<RunCi>, InboxError> {
  let db = ProjectDb::open(root)?;
  let Some(run) = db.run(run).map_err(crate::tasks::detail::run_store_error)? else {
    return Ok(None);
  };
  let Some(pull_request) = stored_run_pull_request(&db, &run)? else {
    return Ok(None);
  };
  drop(db);
  let ci = reads.read((project, root), programs, pull_request.number(), now)?;
  Ok(Some(RunCi { pull_request, ci }))
}

/// Pull request `number` of a project.
type Key = (ProjectId, u64);

/// The reads of the last [`POLL_INTERVAL`], each with when it asked; one per app, shared by its screens and
/// clients.
#[derive(Clone, Default)]
pub struct CiReads(Arc<Mutex<HashMap<Key, (Timestamp, PullRequestCi)>>>);

impl CiReads {
  /// The checks of pull request `number` of `project`, at `root`: what a read less than [`POLL_INTERVAL`] before
  /// `now` gave, else a new read with `programs`. Older reads are forgotten.
  pub fn read(
    &self,
    (project, root): (ProjectId, &Path),
    programs: &Programs,
    number: u64,
    now: Timestamp,
  ) -> Result<PullRequestCi, InboxError> {
    let key = (project, number);
    {
      let mut reads = self.0.lock();
      reads.retain(|_, (at, _)| now < after_poll(*at));
      if let Some((_, kept)) = reads.get(&key) {
        return Ok(kept.clone());
      }
    }
    let read = read_ci_in(root, programs, number, now)?;
    self.0.lock().insert(key, (now, read.clone()));
    Ok(read)
  }

  /// Forgets the read of pull request `number` of `project`, so the next one asks GitHub: *Check again*.
  pub fn forget(&self, project: ProjectId, number: u64) {
    self.0.lock().remove(&(project, number));
  }
}
