//! Saving GitHub connections, pointing a connection at another `gh` program, and making the
//! project expect the account `gh` uses now. Every function blocks on SQLite or on `gh`; the UI
//! calls them off the UI thread. Only the read-only checks of `orchester_core::connection` run.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{ConnectionError, GitHubCheck, GitHubConnection, GitHubHost},
  id::ConnectionId,
  name::Name,
  repository::RepositoryError,
  store::{ConnectionStore, RepositoryStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{data::SaveError, probe};
use crate::messages::{self, Message};

/// A GitHub connection as the add or edit dialog saves it, with the check the dialog ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubDraft {
  /// The connection being edited; `None` adds a new one.
  pub id: Option<ConnectionId>,
  pub name: Name,
  /// Fixed once the connection exists: editing keeps the stored host.
  pub host: GitHubHost,
  pub executable: PathBuf,
  pub check: GitHubCheck,
}

impl lurq::app::component::DevtoolsInspectable for GitHubDraft {}

/// Adds or updates the GitHub connection in `draft` with its check. Saving needs a program that
/// answered as the GitHub CLI; signing in can follow.
pub fn save_github<S>(store: &mut S, draft: GitHubDraft, now: Timestamp) -> Result<ConnectionId, SaveError>
where
  S: ConnectionStore<Error = StoreError>,
{
  if !draft.check.result.is_verified() {
    return Err(SaveError::NotVerified);
  }
  let Some(id) = draft.id else {
    let mut connection =
      GitHubConnection::new(ConnectionId::generate(), draft.name, draft.host, draft.executable, now)?;
    connection.record_check(draft.check);
    store.create_connection(&connection)?;
    return Ok(connection.id());
  };
  let stored = store.connection(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  let mut connection = GitHubConnection::try_from(stored)?;
  connection.rename(draft.name);
  connection.set_executable(draft.executable)?;
  connection.record_check(draft.check);
  store.save_connection(&connection)?;
  Ok(id)
}

pub fn save_github_in(repo_root: &Path, draft: GitHubDraft) -> Result<ConnectionId, SaveError> {
  save_github(&mut ProjectDb::open(repo_root)?, draft, Timestamp::now())
}

/// Points a stored connection at `executable`, checks it and stores the result, whatever it is.
pub fn choose_program_in(repo_root: &Path, id: ConnectionId, executable: PathBuf) -> Result<(), SaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let mut connection = db.connection(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  connection.set_executable(executable)?;
  let check = probe::recheck(&connection).expect("a connection pointed at a program can be checked");
  connection.record_check(check)?;
  db.save_connection(&connection)?;
  Ok(())
}

#[derive(Debug, Error)]
pub enum AccountError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Repository(#[from] RepositoryError),
  #[error(transparent)]
  Connection(#[from] ConnectionError),
  /// The project has no repository link through this connection (any more).
  #[error("the project's repository is not linked through connection {0}")]
  NotLinked(ConnectionId),
}

impl AccountError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::Repository(RepositoryError::NotSignedIn(host)) => {
        Message::new("errors.githubNotSignedIn").with("host", host.to_string())
      }
      Self::Repository(_) => Message::new("errors.githubUnavailable"),
      // The link names a connection that is not a GitHub one: the stored data is inconsistent.
      Self::Connection(_) => Message::new("errors.storage"),
      Self::NotLinked(_) => Message::new("errors.repositoryNotLinked"),
    }
  }
}

/// Checks `gh` of connection `id` again and makes the project expect the account it acts as now
/// (*Use this account*). `gh` itself is never changed.
pub fn expect_active_account_in(repo_root: &Path, id: ConnectionId) -> Result<(), AccountError> {
  let mut db = ProjectDb::open(repo_root)?;
  let connection = github_connection(&db, id)?;
  let check = probe::check_github(connection.host(), connection.executable());
  expect_active_account(&mut db, id, check)
}

/// Stores `check`, a fresh check of connection `id`, whatever it found, then makes the project
/// expect the account it names.
pub fn expect_active_account<S>(store: &mut S, id: ConnectionId, check: GitHubCheck) -> Result<(), AccountError>
where
  S: ConnectionStore<Error = StoreError> + RepositoryStore<Error = StoreError>,
{
  let mut link = store
    .repository_link()?
    .filter(|link| link.connection() == id)
    .ok_or(AccountError::NotLinked(id))?;
  let mut connection = github_connection(store, id)?;
  let result = check.result.clone();
  connection.record_check(check);
  store.save_connection(&connection)?;
  link.expect_account(result.signed_in_account())?;
  store.save_repository_link(&link)?;
  Ok(())
}

fn github_connection<S>(store: &S, id: ConnectionId) -> Result<GitHubConnection, AccountError>
where
  S: ConnectionStore<Error = StoreError>,
{
  let stored = store.connection(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  Ok(GitHubConnection::try_from(stored)?)
}
