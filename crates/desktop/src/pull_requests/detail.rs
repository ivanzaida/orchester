//! One pull request and its diff on this computer (`docs/design/orchester-workspace.md`, "Pull
//! request review", 08.02 `Rrmhf`): what is stored (the inbox's last list and the diff last read
//! for the pull request), and refreshing both through the linked GitHub connection. Every function
//! blocks on SQLite, blob files or `gh`; the UI calls them off the UI thread. Only read-only `gh`
//! commands run.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::GitHubLogin,
  github::GhCli,
  pull_request::{
    DiffRead, PullRequest, RevisionDiff, SyncFailure,
    diff::{Diff, DiffParseError},
    refresh_pull_request,
  },
  repository::{GitHubRepo, RepositoryLink},
  revision::Revision,
  store::{ConnectionStore, PullRequestDiffStore, PullRequestStore, RepositoryStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{InboxError, github_connection};
use crate::{messages::Message, programs::Programs};

/// What the pull request screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Detail {
  /// The project has no linked GitHub repository any more.
  Unlinked,
  Linked(Box<LinkedDetail>),
}

impl lurq::app::component::DevtoolsInspectable for Detail {}

/// A pull request of the linked repository as far as it is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedDetail {
  pub repo: GitHubRepo,
  pub number: u64,
  pub expected: GitHubLogin,
  /// From the latest refresh, else from the inbox's stored list; `None` when neither has it.
  pub pull_request: Option<PullRequest>,
  pub diff: DiffState,
  /// When what is shown was read from GitHub; `None` when nothing was.
  pub synced_at: Option<Timestamp>,
  /// Why the latest refresh did not read the pull request; `None` when it did or before it ran.
  pub failure: Option<SyncFailure>,
}

/// The diff the screen can show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffState {
  /// No diff is stored and none was read.
  Unknown,
  Read(Arc<ReadDiff>),
  /// The pull request is too large to read here (see `is_too_large`).
  TooLarge,
}

/// A diff read for one head revision, parsed.
#[derive(Debug, PartialEq, Eq)]
pub struct ReadDiff {
  pub base: Revision,
  pub head: Revision,
  pub synced_at: Timestamp,
  pub diff: Diff,
}

impl ReadDiff {
  fn parse(stored: RevisionDiff) -> Result<Self, DetailError> {
    Ok(Self {
      diff: Diff::parse(&stored.text)?,
      base: stored.base,
      head: stored.head,
      synced_at: stored.synced_at,
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
  let db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(Detail::Unlinked);
  };
  stored(&db, &link, number, None)
}

/// Refreshes pull request `number` and its diff with `programs`, storing a diff of a new head,
/// together with the check of `gh` it made on the link's connection.
pub fn refresh_detail_in(repo_root: &Path, number: u64, programs: &Programs) -> Result<Detail, DetailError> {
  let mut db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(Detail::Unlinked);
  };
  let mut connection = github_connection(&db, link.connection())?;
  let github = GhCli::new(programs.runner(), &connection);
  let now = Timestamp::now();
  let refresh = refresh_pull_request(&github, &mut db, &link, number, now)?;
  connection.record_check(refresh.check);
  db.save_connection(&connection)?;
  match refresh.outcome {
    Ok(read) => {
      let diff = match read.diff {
        DiffRead::Read(diff) => DiffState::Read(Arc::new(ReadDiff::parse(diff)?)),
        DiffRead::TooLarge => DiffState::TooLarge,
      };
      Ok(Detail::Linked(Box::new(LinkedDetail {
        repo: link.repo().clone(),
        number,
        expected: link.expected_login().clone(),
        pull_request: Some(read.pull_request),
        diff,
        synced_at: Some(now),
        failure: None,
      })))
    }
    Err(failure) => {
      tracing::info!(repository = %link.repo(), number, ?failure, "the pull request was not refreshed");
      stored(&db, &link, number, Some(failure))
    }
  }
}

/// What storage knows about pull request `number`, and why the latest refresh failed.
fn stored(
  db: &ProjectDb,
  link: &RepositoryLink,
  number: u64,
  failure: Option<SyncFailure>,
) -> Result<Detail, DetailError> {
  let repo = link.repo();
  let pull_request = db.pull_requests(repo)?.and_then(|synced| {
    synced
      .pull_requests
      .into_iter()
      .find(|pull_request| pull_request.number() == number)
  });
  let diff = match db.latest_pull_request_diff(repo, number)? {
    Some(stored) => DiffState::Read(Arc::new(ReadDiff::parse(stored)?)),
    None => DiffState::Unknown,
  };
  let synced_at = match &diff {
    DiffState::Read(read) => Some(read.synced_at),
    DiffState::Unknown | DiffState::TooLarge => None,
  };
  Ok(Detail::Linked(Box::new(LinkedDetail {
    repo: repo.clone(),
    number,
    expected: link.expected_login().clone(),
    pull_request,
    diff,
    synced_at,
    failure,
  })))
}
