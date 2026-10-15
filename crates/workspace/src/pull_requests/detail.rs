//! One pull request and its diff on this computer (`docs/design/orchester-workspace.md`, "Pull
//! request review", 08.02 `Rrmhf`): what is stored (the inbox's last list and the diff last read
//! for the pull request), and refreshing both through the linked GitHub connection. Every function
//! blocks on SQLite, blob files or `gh`; the UI calls them off the UI thread. Only read-only `gh`
//! commands run.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::GitHubLogin,
  diff::{Diff, DiffParseError, RevisionDiff},
  github::GhCli,
  pull_request::{DiffRead, Fields, PullRequest, SyncFailure, refresh_pull_request},
  repository::{GitHubRepo, RepositoryLink},
  revision::Revision,
  store::{ConnectionStore, PullRequestDiffStore, PullRequestStore, RepositoryStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{
  InboxError, github_connection,
  linked::{LinkedTask, linked_tasks},
};
use crate::messages::Message;

/// What the pull request screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Detail {
  /// The project has no linked GitHub repository any more.
  Unlinked,
  Linked(Box<LinkedDetail>),
}

/// A pull request of the linked repository as far as it is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedDetail {
  pub repo: GitHubRepo,
  pub number: u64,
  pub expected: GitHubLogin,
  /// From the latest refresh, else from the inbox's stored list; `None` when neither has it.
  pub pull_request: Option<PullRequest>,
  /// The task whose run made its branch ([`super::linked`]).
  pub task: Option<LinkedTask>,
  pub diff: DiffState,
  /// When what is shown was read from GitHub; `None` when nothing was.
  pub synced_at: Option<Timestamp>,
  /// Why the latest refresh did not read the pull request; `None` when it did or before it ran.
  pub failure: Option<SyncFailure>,
  /// Why the latest refresh read the pull request but not the diff of its head; the diff shown,
  /// if any, is the one stored last.
  pub diff_failure: Option<SyncFailure>,
}

impl LinkedDetail {
  /// What the latest refresh could not read, the pull request or else its diff.
  pub fn latest_failure(&self) -> Option<&SyncFailure> {
    self.failure.as_ref().or(self.diff_failure.as_ref())
  }
}

/// The diff the screen can show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffState {
  /// No diff is stored and none was read.
  Unknown,
  Read(Arc<ReadDiff>),
  /// The pull request is too large to read here (see `Changes::is_too_large`).
  TooLarge,
}

/// A diff read for one head revision, parsed.
#[derive(Debug, PartialEq, Eq)]
pub struct ReadDiff {
  pub base: Revision,
  pub head: Revision,
  pub synced_at: Timestamp,
  pub diff: Arc<Diff>,
}

impl ReadDiff {
  pub(super) fn parse(stored: RevisionDiff) -> Result<Self, DiffParseError> {
    Ok(Self {
      diff: Arc::new(Diff::parse(&stored.text)?),
      base: stored.base,
      head: stored.head,
      synced_at: stored.read_at,
    })
  }
}

#[derive(Debug, Error)]
pub enum DetailError {
  #[error(transparent)]
  Inbox(#[from] InboxError),
  #[error("the stored or read diff is not a unified diff: {0}")]
  Diff(#[from] DiffParseError),
}

impl From<StoreError> for DetailError {
  fn from(error: StoreError) -> Self {
    Self::Inbox(InboxError::Store(error))
  }
}

impl DetailError {
  pub fn message(&self) -> Message {
    match self {
      Self::Inbox(error) => error.message(),
      Self::Diff(_) => Message::new("pullRequests.detail.diffUnreadable"),
    }
  }
}

/// Pull request `number` as stored, without asking GitHub.
pub fn load_detail_in(repo_root: &Path, number: u64) -> Result<Detail, DetailError> {
  detail(find_in(repo_root, number)?)
}

/// Refreshes pull request `number` and its diff with `programs`, storing a diff of a new head,
/// together with the check of `gh` it made on the link's connection. While a rate limit the pull requests'
/// sync recorded has not reset, nothing asks GitHub: what is stored comes back with the rate limit.
pub fn refresh_detail_in(repo_root: &Path, number: u64, programs: &Programs) -> Result<Detail, DetailError> {
  detail(refresh_found_in(repo_root, (number, Fields::Inbox), programs)?)
}

/// What a read of a pull request found, its diff's text not read as a diff yet; `None` without a link.
pub(super) struct Found {
  pub repo: GitHubRepo,
  pub number: u64,
  pub expected: GitHubLogin,
  pub pull_request: Option<PullRequest>,
  pub task: Option<LinkedTask>,
  pub diff: FoundDiff,
  pub synced_at: Option<Timestamp>,
  pub failure: Option<SyncFailure>,
  pub diff_failure: Option<SyncFailure>,
}

pub(super) enum FoundDiff {
  Unknown,
  Text(RevisionDiff),
  TooLarge,
}

fn detail(found: Option<Found>) -> Result<Detail, DetailError> {
  let Some(found) = found else {
    return Ok(Detail::Unlinked);
  };
  let diff = match found.diff {
    FoundDiff::Unknown => DiffState::Unknown,
    FoundDiff::TooLarge => DiffState::TooLarge,
    FoundDiff::Text(text) => DiffState::Read(Arc::new(ReadDiff::parse(text)?)),
  };
  Ok(Detail::Linked(Box::new(LinkedDetail {
    repo: found.repo,
    number: found.number,
    expected: found.expected,
    pull_request: found.pull_request,
    task: found.task,
    diff,
    synced_at: found.synced_at,
    failure: found.failure,
    diff_failure: found.diff_failure,
  })))
}

pub(super) fn find_in(repo_root: &Path, number: u64) -> Result<Option<Found>, InboxError> {
  let db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(None);
  };
  stored(&db, &link, number, None).map(Some)
}

/// Reads pull request `number` and its head's diff from GitHub, the pull request with `fields`.
pub(super) fn refresh_found_in(
  repo_root: &Path,
  (number, fields): (u64, Fields),
  programs: &Programs,
) -> Result<Option<Found>, InboxError> {
  let mut db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(None);
  };
  let now = Timestamp::now();
  if let Some(attempt) = db.pull_request_sync(link.repo())?
    && attempt.is_paused(now)
  {
    return stored(&db, &link, number, attempt.failure).map(Some);
  }
  let mut connection = github_connection(&db, link.connection())?;
  let github = GhCli::new(programs.runner(), &connection);
  let refresh = refresh_pull_request(&github, &mut db, &link, (number, fields), now)?;
  connection.record_check(refresh.check);
  db.save_connection(&connection)?;
  let read = match refresh.outcome {
    Ok(read) => read,
    Err(failure) => {
      tracing::info!(repository = %link.repo(), number, ?failure, "the pull request was not refreshed");
      return stored(&db, &link, number, Some(failure)).map(Some);
    }
  };
  let (diff, diff_failure) = match read.diff {
    DiffRead::Read(diff) => (FoundDiff::Text(diff), None),
    DiffRead::TooLarge => (FoundDiff::TooLarge, None),
    DiffRead::Failed(failure) => {
      tracing::info!(repository = %link.repo(), number, ?failure, "the diff of the pull request was not read");
      (stored_diff(&db, &link, number)?, Some(failure))
    }
  };
  Ok(Some(Found {
    repo: link.repo().clone(),
    number,
    expected: link.expected_login().clone(),
    task: task_of(&db, Some(&read.pull_request))?,
    pull_request: Some(read.pull_request),
    diff,
    synced_at: Some(now),
    failure: None,
    diff_failure,
  }))
}

/// What storage knows about pull request `number`, and why the latest refresh failed.
fn stored(
  db: &ProjectDb,
  link: &RepositoryLink,
  number: u64,
  failure: Option<SyncFailure>,
) -> Result<Found, InboxError> {
  let repo = link.repo();
  let pull_request = db.pull_requests(repo)?.and_then(|synced| {
    synced
      .pull_requests
      .into_iter()
      .find(|pull_request| pull_request.number() == number)
  });
  let diff = stored_diff(db, link, number)?;
  let synced_at = match &diff {
    FoundDiff::Text(text) => Some(text.read_at),
    FoundDiff::Unknown | FoundDiff::TooLarge => None,
  };
  Ok(Found {
    repo: repo.clone(),
    number,
    expected: link.expected_login().clone(),
    task: task_of(db, pull_request.as_ref())?,
    pull_request,
    diff,
    synced_at,
    failure,
    diff_failure: None,
  })
}

/// The task of `pull_request`, if it has one.
fn task_of(db: &ProjectDb, pull_request: Option<&PullRequest>) -> Result<Option<LinkedTask>, InboxError> {
  let Some(pull_request) = pull_request else {
    return Ok(None);
  };
  Ok(linked_tasks(db, std::slice::from_ref(pull_request))?.remove(&pull_request.number()))
}

/// The diff last stored for pull request `number`.
fn stored_diff(db: &ProjectDb, link: &RepositoryLink, number: u64) -> Result<FoundDiff, InboxError> {
  Ok(match db.latest_pull_request_diff(link.repo(), number)? {
    Some(stored) => FoundDiff::Text(stored),
    None => FoundDiff::Unknown,
  })
}
