use std::{io, path::PathBuf};

use orchester_core::{
  id::{ConnectionId, InvocationId, ResourceId, RunId, SecretId, TaskId},
  resource::{PublishError, ResourceKind},
  run::RunError,
  task::EditError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
  #[error("database error: {0}")]
  Sqlite(#[from] rusqlite::Error),
  #[error("cannot access {path}: {source}")]
  Io { path: PathBuf, source: io::Error },
  #[error("{0} is not the root of a git repository")]
  NotAGitRepository(PathBuf),
  #[error("{0} already contains an Orchester project")]
  AlreadyInitialized(PathBuf),
  #[error("{0} does not contain an Orchester project")]
  NotInitialized(PathBuf),
  #[error("database schema version {found} is newer than this build supports ({supported})")]
  SchemaTooNew { found: u32, supported: u32 },
  /// A database opened only to read it has not been migrated to this build's schema yet.
  #[error("database schema version {found} is older than this build's ({supported}) and was not migrated")]
  SchemaTooOld { found: u32, supported: u32 },
  /// A migration left rows referring to rows that do not exist; nothing was changed.
  #[error("migrating the database would leave {0} broken references; it was left unchanged")]
  MigrationBrokeReferences(i64),
  /// A stored value no longer satisfies the domain rules; the database was edited or damaged.
  #[error("stored {what} is invalid: {detail}")]
  Corrupt { what: &'static str, detail: String },
  /// A value cannot be represented in the database, e.g. a non-UTF-8 path.
  #[error("{0} cannot be stored")]
  Unrepresentable(&'static str),
  #[error("task {0} does not exist")]
  TaskNotFound(TaskId),
  /// The task can no longer be edited; nothing was changed.
  #[error("task {task} cannot be edited: {source}")]
  TaskNotEditable { task: TaskId, source: EditError },
  #[error("a task batch must contain at least one task")]
  EmptyBatch,
  #[error("task {0} no longer uses its reviewed pipeline; review again")]
  TaskGroupReviewChanged(TaskId),
  #[error("resource {0} does not exist")]
  ResourceNotFound(ResourceId),
  #[error("resource {resource} is a {found:?}, not a {expected:?}")]
  ResourceKindMismatch {
    resource: ResourceId,
    expected: ResourceKind,
    found: ResourceKind,
  },
  #[error("a {kind:?} with slug `{slug}` already exists")]
  DuplicateSlug { kind: ResourceKind, slug: String },
  #[error("resource {0} has no draft to publish")]
  NoDraft(ResourceId),
  #[error(transparent)]
  Publish(#[from] PublishError),
  #[error("a connection named `{0}` already exists")]
  DuplicateConnectionName(String),
  #[error("connection {0} does not exist")]
  ConnectionNotFound(ConnectionId),
  #[error("connection {0} is not a GitHub connection")]
  NotAGitHubConnection(ConnectionId),
  #[error("invocation {0} does not exist")]
  InvocationNotFound(InvocationId),
  #[error("a secret named `{name}` already exists in {environment}")]
  DuplicateSecretName { name: String, environment: &'static str },
  #[error("secret {0} does not exist")]
  SecretNotFound(SecretId),
  #[error("run {0} was merged already")]
  AlreadyMerged(RunId),
}

impl StoreError {
  pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
    let path = path.into();
    move |source| Self::Io { path, source }
  }
}

/// Errors of the run store: storage failures plus the rules of appending to a run's log.
#[derive(Debug, Error)]
pub enum RunStoreError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Run(#[from] RunError),
  #[error("run {0} does not exist")]
  RunNotFound(RunId),
  /// The stored record of run `run` cannot be read back: it breaks the run's rules or a value in
  /// it cannot be decoded. The record is kept unchanged.
  #[error("run {run} cannot be read: {source}")]
  Unreadable { run: RunId, source: StoreError },
  /// Another writer appended to the run's log first; reload the run and retry the transition.
  #[error("run {run} expected event {expected} next, got {found}")]
  SequenceConflict { run: RunId, expected: u64, found: u64 },
  /// The events handed to the store do not lead to the state being stored.
  #[error("events do not match the run being stored: {0}")]
  EventsMismatch(&'static str),
}

impl From<rusqlite::Error> for RunStoreError {
  fn from(error: rusqlite::Error) -> Self {
    Self::Store(StoreError::Sqlite(error))
  }
}
