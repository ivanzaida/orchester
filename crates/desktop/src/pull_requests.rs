//! The project's pull request inbox on this computer (`docs/design/orchester-workspace.md`,
//! "Pull requests" and "GitHub", rule 9): the list stored by the last sync, and syncing it
//! through the linked GitHub connection. Every function blocks on SQLite or `gh`; the UI calls
//! them off the UI thread. Only read-only `gh` commands run.

use std::path::Path;

use orchester_core::{
  connection::{ConnectionError, GitHubConnection, GitHubLogin, check_github},
  github::GhCli,
  id::ConnectionId,
  pull_request::{SyncFailure, SyncedPullRequests, sync_pull_requests},
  repository::GitHubRepo,
  store::{ConnectionStore, PullRequestStore, RepositoryStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use crate::{
  connections::github::{AccountError, expect_active_account},
  messages::{self, Message},
  programs::Programs,
};

/// What the Pull requests screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inbox {
  /// The project has no linked GitHub repository: pull requests are unavailable (08.D).
  Unlinked,
  Linked(Box<LinkedInbox>),
}

impl lurq::app::component::DevtoolsInspectable for Inbox {}

/// The linked repository's pull requests as far as they are known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedInbox {
  pub repo: GitHubRepo,
  pub connection: ConnectionId,
  pub expected: GitHubLogin,
  /// The list the last successful sync stored; `None` before the first one.
  pub synced: Option<SyncedPullRequests>,
  /// Why the latest sync of this screen did not read the list; `None` when it did or before it
  /// ran.
  pub failure: Option<SyncFailure>,
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

/// The inbox as stored, without asking GitHub.
pub fn load_in(repo_root: &Path) -> Result<Inbox, InboxError> {
  let db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(Inbox::Unlinked);
  };
  Ok(Inbox::Linked(Box::new(LinkedInbox {
    synced: db.pull_requests(link.repo())?,
    repo: link.repo().clone(),
    connection: link.connection(),
    expected: link.expected_login().clone(),
    failure: None,
  })))
}

/// Syncs the open pull requests of the linked repository with `programs` and stores what it
/// read, together with the check of `gh` it made on the link's connection.
pub fn sync_in(repo_root: &Path, programs: &Programs) -> Result<Inbox, InboxError> {
  let mut db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(Inbox::Unlinked);
  };
  let mut connection = github_connection(&db, link.connection())?;
  let now = Timestamp::now();
  let result = sync_pull_requests(&GhCli::new(programs.runner(), &connection), &link, now);
  connection.record_check(result.check);
  db.save_connection(&connection)?;
  let failure = match result.outcome {
    Ok(pull_requests) => {
      db.replace_pull_requests(link.repo(), &pull_requests, now)?;
      None
    }
    Err(failure) => {
      tracing::info!(repository = %link.repo(), ?failure, "pull requests were not synced");
      Some(failure)
    }
  };
  Ok(Inbox::Linked(Box::new(LinkedInbox {
    synced: db.pull_requests(link.repo())?,
    repo: link.repo().clone(),
    connection: link.connection(),
    expected: link.expected_login().clone(),
    failure,
  })))
}

/// *Use this account* (08.G): checks `gh` of the link's connection with `programs`, makes the
/// project expect the account it acts as now, then syncs. `gh` itself is never changed.
pub fn use_active_account_in(repo_root: &Path, programs: &Programs) -> Result<Inbox, InboxError> {
  {
    let mut db = ProjectDb::open(repo_root)?;
    let Some(link) = db.repository_link()? else {
      return Ok(Inbox::Unlinked);
    };
    let connection = github_connection(&db, link.connection())?;
    let check = check_github(
      connection.executable(),
      connection.host(),
      &programs.runner(),
      Timestamp::now(),
    );
    expect_active_account(&mut db, link.connection(), check)?;
  }
  sync_in(repo_root, programs)
}

fn github_connection(db: &ProjectDb, id: ConnectionId) -> Result<GitHubConnection, InboxError> {
  let stored = db.connection(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  Ok(GitHubConnection::try_from(stored)?)
}
