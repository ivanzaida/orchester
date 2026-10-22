//! The project's pull request inbox on this computer (`docs/design/orchester-workspace.md`,
//! "Pull requests" and "GitHub", rule 9): the list stored by the last sync, and syncing it
//! through the linked GitHub connection. Every function blocks on SQLite or `gh`; the UI calls
//! them off the UI thread. Only read-only `gh` commands run, except where a person creates a pull
//! request from a run ([`create`]).
//!
//! The deprecated desktop's inbox ([`load_in`], [`sync_in`]) holds the open pull requests; the new
//! app's page ([`list`]) syncs the closed and merged ones too, into the same stored list.

pub mod create;
pub mod detail;
pub mod feedback;
pub mod linked;
pub mod list;
pub mod page;
pub mod status;
pub mod unpushed;

use std::path::Path;

use orchester_core::{
  connection::{ConnectionError, GitHubConnection, GitHubLogin, check_github},
  github::GhCli,
  id::ConnectionId,
  pull_request::{
    PullRequest, PullRequestState, SyncAttempt, SyncFailure, SyncScope, SyncedPullRequests, sort_by_update,
    sync_pull_requests,
  },
  repository::{GitHubRepo, RepositoryLink},
  store::{ConnectionStore, PullRequestStore, RepositoryStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use crate::{
  connections::github::{AccountError, expect_active_account},
  messages::{self, Message},
};

/// What the Pull requests screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inbox {
  /// The project has no linked GitHub repository: pull requests are unavailable (08.D).
  Unlinked,
  Linked(Box<LinkedInbox>),
}

/// The linked repository's pull requests as far as they are known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedInbox {
  pub repo: GitHubRepo,
  pub connection: ConnectionId,
  pub expected: GitHubLogin,
  /// The list the last successful sync stored; `None` before the first one.
  pub synced: Option<SyncedPullRequests>,
  /// Why the latest sync did not read the list; `None` when it did or before one ran.
  pub failure: Option<SyncFailure>,
  /// When the latest sync ran, whether it read the list or not; `None` before the first.
  pub attempted_at: Option<Timestamp>,
}

#[derive(Debug, Error)]
pub enum InboxError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Connection(#[from] ConnectionError),
  #[error(transparent)]
  Account(#[from] AccountError),
}

impl InboxError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      // The link names a connection that is not a GitHub one: the stored data is inconsistent.
      Self::Connection(_) => Message::new("errors.storage"),
      Self::Account(error) => error.message(),
    }
  }
}

impl Inbox {
  /// The inbox with only its open pull requests, drafts included.
  #[must_use]
  pub fn open_only(mut self) -> Self {
    if let Self::Linked(linked) = &mut self
      && let Some(synced) = &mut linked.synced
    {
      synced
        .pull_requests
        .retain(|pull_request| matches!(pull_request.state(), PullRequestState::Open | PullRequestState::Draft));
    }
    self
  }
}

/// What the status bar says of a sync that ended without reading the list: GitHub refused it, and the page
/// says why.
pub fn not_synced() -> Message {
  Message::new("pullRequests.sync.notSynced")
}

/// Why a pull request's address did not reach the browser.
pub fn open_failed() -> Message {
  Message::new("pullRequests.openFailed")
}

/// The open pull requests as stored, without asking GitHub. The deprecated desktop's screen reports only the
/// syncs it ran itself: a failure recorded before it opened is not its.
pub fn load_in(repo_root: &Path) -> Result<Inbox, InboxError> {
  let mut inbox = stored_in(repo_root)?.open_only();
  if let Inbox::Linked(linked) = &mut inbox {
    linked.failure = None;
  }
  Ok(inbox)
}

/// Every pull request as stored, without asking GitHub.
fn stored_in(repo_root: &Path) -> Result<Inbox, InboxError> {
  stored(&ProjectDb::open(repo_root)?)
}

/// Every pull request `db` stores.
pub(crate) fn stored(db: &ProjectDb) -> Result<Inbox, InboxError> {
  let Some(link) = db.repository_link()? else {
    return Ok(Inbox::Unlinked);
  };
  linked(db, &link)
}

/// The stored list of `link`'s repository with how its latest sync went.
fn linked(db: &ProjectDb, link: &RepositoryLink) -> Result<Inbox, InboxError> {
  let attempt = db.pull_request_sync(link.repo())?;
  Ok(Inbox::Linked(Box::new(LinkedInbox {
    synced: db.pull_requests(link.repo())?,
    repo: link.repo().clone(),
    connection: link.connection(),
    expected: link.expected_login().clone(),
    attempted_at: attempt.as_ref().map(|attempt| attempt.at),
    failure: attempt.and_then(|attempt| attempt.failure),
  })))
}

/// Syncs the open pull requests of the linked repository with `programs` and stores what it
/// read, together with the check of `gh` it made on the link's connection.
pub fn sync_in(repo_root: &Path, programs: &Programs) -> Result<Inbox, InboxError> {
  Ok(sync_scope_in(repo_root, programs, SyncScope::Open)?.open_only())
}

/// Syncs the pull requests of `scope` with `programs` and stores what it read with how it went, together with
/// the check of `gh` it made on the link's connection. While a recorded rate limit has not reset, nothing
/// asks GitHub: the stored list comes back with the rate limit. A sync of the open ones keeps the stored
/// closed and merged ones it did not read.
pub(crate) fn sync_scope_in(repo_root: &Path, programs: &Programs, scope: SyncScope) -> Result<Inbox, InboxError> {
  let mut db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(Inbox::Unlinked);
  };
  let now = Timestamp::now();
  if db
    .pull_request_sync(link.repo())?
    .is_some_and(|attempt| attempt.is_paused(now))
  {
    return linked(&db, &link);
  }
  let mut connection = github_connection(&db, link.connection())?;
  let result = sync_pull_requests(&GhCli::new(programs.runner(), &connection), &link, scope, now);
  connection.record_check(result.check);
  db.save_connection(&connection)?;
  let failure = match result.outcome {
    Ok(pull_requests) => {
      let pull_requests = match scope {
        SyncScope::OpenAndClosed => pull_requests,
        SyncScope::Open => with_stored_closed(&db, link.repo(), pull_requests)?,
      };
      db.replace_pull_requests(link.repo(), &pull_requests, now)?;
      None
    }
    Err(failure) => {
      tracing::info!(repository = %link.repo(), ?failure, "pull requests were not synced");
      Some(failure)
    }
  };
  db.record_pull_request_sync(link.repo(), &SyncAttempt { at: now, failure })?;
  linked(&db, &link)
}

/// `open`, read by a sync of the open ones, and the stored closed and merged ones it did not read.
fn with_stored_closed(
  db: &ProjectDb,
  repo: &GitHubRepo,
  mut open: Vec<PullRequest>,
) -> Result<Vec<PullRequest>, InboxError> {
  let stored = db
    .pull_requests(repo)?
    .map(|synced| synced.pull_requests)
    .unwrap_or_default();
  let closed: Vec<PullRequest> = stored
    .into_iter()
    .filter(|stored| {
      matches!(stored.state(), PullRequestState::Merged | PullRequestState::Closed)
        && open.iter().all(|read| read.number() != stored.number())
    })
    .collect();
  open.extend(closed);
  sort_by_update(&mut open);
  Ok(open)
}

/// *Use this account* (08.G): checks `gh` of the link's connection with `programs`, makes the
/// project expect the account it acts as now, then syncs. `gh` itself is never changed.
pub fn use_active_account_in(repo_root: &Path, programs: &Programs) -> Result<Inbox, InboxError> {
  if !expect_active_account_in(repo_root, programs)? {
    return Ok(Inbox::Unlinked);
  }
  sync_in(repo_root, programs)
}

/// Makes the project expect the account `gh` of the link's connection acts as now; `false` when the
/// project has no link.
pub(crate) fn expect_active_account_in(repo_root: &Path, programs: &Programs) -> Result<bool, InboxError> {
  let mut db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(false);
  };
  let connection = github_connection(&db, link.connection())?;
  let check = check_github(
    connection.executable(),
    connection.host(),
    &programs.runner(),
    Timestamp::now(),
  );
  expect_active_account(&mut db, link.connection(), check)?;
  Ok(true)
}

pub(crate) fn github_connection(db: &ProjectDb, id: ConnectionId) -> Result<GitHubConnection, InboxError> {
  let stored = db.connection(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  Ok(GitHubConnection::try_from(stored)?)
}
