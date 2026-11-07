//! A run's pull request on this computer (`docs/design/orchester-workspace.md`, "GitHub", rule 9): creating it
//! from a run ([`create`]) and whether it would carry the local base's unpushed commits ([`unpushed`]), its CI
//! ([`ci`]) and *Fix CI* ([`ci_fix`]), all over the list the last sync of the linked GitHub repository stored. Every
//! function blocks on SQLite or `gh`; the UI calls them off the UI thread. Only read-only `gh` commands run, except
//! where a person creates a pull request from a run.

pub mod ci;
pub mod ci_fix;
pub mod create;
pub mod unpushed;

use std::path::Path;

use orchester_core::{
  connection::{ConnectionError, GitHubConnection},
  github::GhCli,
  id::ConnectionId,
  pull_request::{SyncAttempt, SyncFailure, SyncScope, sync_pull_requests},
  repository::GitHubRepo,
  store::{ConnectionStore, PullRequestStore, RepositoryStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use crate::{
  connections::github::AccountError,
  messages::{self, Message},
};

#[derive(Debug, Error)]
pub enum PullRequestError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Connection(#[from] ConnectionError),
  #[error(transparent)]
  Account(#[from] AccountError),
}

impl PullRequestError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      // The link names a connection that is not a GitHub one: the stored data is inconsistent.
      Self::Connection(_) => Message::new("errors.storage"),
      Self::Account(error) => error.message(),
    }
  }
}

/// Syncs the open, closed and merged pull requests of the linked repository with `programs` and stores what it
/// read with how it went, together with the check of `gh` it made on the link's connection. While a recorded rate
/// limit has not reset, nothing asks GitHub. A project without a link has nothing to sync.
pub(crate) fn sync_in(repo_root: &Path, programs: &Programs) -> Result<(), PullRequestError> {
  let mut db = ProjectDb::open(repo_root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(());
  };
  let now = Timestamp::now();
  if db
    .pull_request_sync(link.repo())?
    .is_some_and(|attempt| attempt.is_paused(now))
  {
    return Ok(());
  }
  let mut connection = github_connection(&db, link.connection())?;
  let result = sync_pull_requests(
    &GhCli::new(programs.runner(), &connection),
    &link,
    SyncScope::OpenAndClosed,
    now,
  );
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
  db.record_pull_request_sync(link.repo(), &SyncAttempt { at: now, failure })?;
  Ok(())
}

/// Records a rate limit that refused a read of one pull request as the repository's latest sync, so every later
/// sync and read waits for the reset too. Any other failure concerns this read alone.
pub(crate) fn hold_if_rate_limited(
  db: &mut ProjectDb,
  repo: &GitHubRepo,
  failure: &SyncFailure,
  now: Timestamp,
) -> Result<(), PullRequestError> {
  if matches!(failure, SyncFailure::RateLimited { .. }) {
    let attempt = SyncAttempt {
      at: now,
      failure: Some(failure.clone()),
    };
    db.record_pull_request_sync(repo, &attempt)?;
  }
  Ok(())
}

pub(crate) fn github_connection(db: &ProjectDb, id: ConnectionId) -> Result<GitHubConnection, PullRequestError> {
  let stored = db.connection(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  Ok(GitHubConnection::try_from(stored)?)
}
