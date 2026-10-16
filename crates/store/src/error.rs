use std::{io, path::PathBuf};

use orchester_core::{
  id::{
    AgentAccountId, ConnectionId, InvocationId, MemoryId, ProjectId, ResourceId, RunId, SecretId, ServerId, TaskId,
  },
  resource::{PublishError, ResourceKind},
  run::{AlwaysAllowError, RunError},
  task::EditError,
  task_selection::SelectionProblem,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
  #[error(transparent)]
  Organization(#[from] orchester_core::task::organization::OrganizationError),
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
  /// The project is another Orchester Cloud account's on this computer: it is listed and opened only for that account.
  /// Nothing was changed.
  #[error("project {0} is another account's")]
  ProjectOfAnotherAccount(ProjectId),
  #[error("database schema version {found} is newer than this build supports ({supported})")]
  SchemaTooNew { found: u32, supported: u32 },
  /// A database opened only to read it has not been migrated to this build's schema yet.
  #[error("database schema version {found} is older than this build's ({supported}) and was not migrated")]
  SchemaTooOld { found: u32, supported: u32 },
  /// Migrating from schema version `found` to `supported` left `references` rows referring to rows
  /// that do not exist; nothing was changed.
  #[error(
    "migrating the database from version {found} to {supported} would leave {references} broken references; it was \
     left unchanged"
  )]
  MigrationBrokeReferences {
    references: i64,
    found: u32,
    supported: u32,
  },
  /// A stored value no longer satisfies the domain rules; the database was edited or damaged.
  #[error("stored {what} is invalid: {detail}")]
  Corrupt { what: &'static str, detail: String },
  /// A value cannot be represented in the database, e.g. a number beyond its 64-bit integers.
  #[error("{0} cannot be stored")]
  Unrepresentable(&'static str),
  /// A path cannot be represented in the database: it is not UTF-8.
  #[error("{0} cannot be stored: the path is not UTF-8")]
  UnrepresentablePath(&'static str),
  #[error("task {0} does not exist")]
  TaskNotFound(TaskId),
  /// The task can no longer be edited; nothing was changed.
  #[error("task {task} cannot be edited: {source}")]
  TaskNotEditable { task: TaskId, source: EditError },
  #[error("a task batch must contain at least one task")]
  EmptyBatch,
  /// The task is a shared task: it changes on the server, and the cache only follows what the
  /// server confirmed. Nothing was changed.
  #[error("task {0} is shared; it changes on the server")]
  SharedTask(TaskId),
  /// The project's task list is shared: its new tasks and its tags are made on the server.
  /// Nothing was changed.
  #[error("the project's task list is shared; this change is made on the server")]
  SharedProject,
  /// The task was kept *Not shared* when the project joined a server project: it is read-only until
  /// *Share task* uploads it. Nothing was changed.
  #[error("task {0} is not shared; it is read-only")]
  NotSharedTask(TaskId),
  /// The project's task list is local: it has no server project.
  #[error("the project's task list is not shared")]
  NotShared,
  /// The project is shared already; nothing was changed.
  #[error("the project's task list is shared already")]
  AlreadyShared,
  #[error("task {0} no longer uses its reviewed pipeline; review again")]
  TaskGroupReviewChanged(TaskId),
  /// A task of a group being queued is not a draft ready for its first run; nothing was queued.
  #[error("task {task} cannot join a group: {problem:?}")]
  TaskNotSelectable { task: TaskId, problem: SelectionProblem },
  /// The pipeline has no published version to assign; nothing was changed.
  #[error("pipeline {0} has no published version")]
  PipelineNotPublished(ResourceId),
  /// The task is not an unstarted member of a group that holds its work, so it stays in its group;
  /// nothing was changed.
  #[error("task {0} is not held by its group")]
  TaskNotHeld(TaskId),
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
  /// The draft of resource `0` is not the one the caller checked: it changed meanwhile.
  #[error("the draft of resource {0} changed since it was read")]
  DraftChanged(ResourceId),
  #[error(transparent)]
  Publish(#[from] PublishError),
  #[error(transparent)]
  AlwaysAllow(#[from] AlwaysAllowError),
  #[error("a connection named `{0}` already exists")]
  DuplicateConnectionName(String),
  #[error("connection {0} does not exist")]
  ConnectionNotFound(ConnectionId),
  #[error("connection {0} is not a GitHub connection")]
  NotAGitHubConnection(ConnectionId),
  #[error("connection {0} is not an agent CLI connection")]
  NotAnAgentCliConnection(ConnectionId),
  #[error("invocation {0} does not exist")]
  InvocationNotFound(InvocationId),
  #[error("invocation {0} is not an open agent launch without a recorded permission snapshot")]
  InvocationPermissionsNotRecordable(InvocationId),
  #[error("a secret named `{name}` already exists in {environment}")]
  DuplicateSecretName { name: String, environment: &'static str },
  #[error("secret {0} does not exist")]
  SecretNotFound(SecretId),
  /// The application database has no agent account with this id, or only a removed one.
  #[error("agent account {0} is not in the list")]
  AgentAccountNotFound(AgentAccountId),
  /// Another account of the CLI has the name already.
  #[error("an account named `{0}` already exists")]
  DuplicateAccountName(String),
  /// Another account has the folder already.
  #[error("another account uses the folder already")]
  AccountFolderTaken,
  /// The application database has no shared server with this id.
  #[error("server {0} is not in the list")]
  ServerNotFound(ServerId),
  #[error("run {0} was merged already")]
  AlreadyMerged(RunId),
  #[error("run {0} does not exist")]
  RunNotFound(RunId),
  #[error("memory {0} does not exist")]
  MemoryNotFound(MemoryId),
  /// The stored record changed since this copy of it was read; read it again. Nothing was changed.
  #[error("memory {0} changed since it was read")]
  MemoryConflict(MemoryId),
  /// A record with the id is stored already; nothing was changed.
  #[error("memory {0} exists already")]
  MemoryExists(MemoryId),
  /// The stored record is no longer active, so it cannot be superseded; nothing was changed.
  #[error("memory {0} is no longer active")]
  MemoryNotActive(MemoryId),
  /// Another active record of the scope has the key; nothing was changed.
  #[error("an active memory record with the key `{0}` already exists")]
  DuplicateMemoryKey(String),
}

impl StoreError {
  /// Whether another database connection holds a lock that prevents this operation.
  pub fn is_locked(&self) -> bool {
    matches!(
      self,
      Self::Sqlite(rusqlite::Error::SqliteFailure(failure, _))
        if matches!(failure.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
  }

  /// Whether the database file is damaged or is not a database at all, as opposed to one that is busy,
  /// refused or cannot be reached: what it holds cannot be trusted.
  pub fn is_damaged(&self) -> bool {
    match self {
      Self::Corrupt { .. } => true,
      Self::Sqlite(rusqlite::Error::SqliteFailure(failure, _)) => matches!(
        failure.code,
        rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
      ),
      _ => false,
    }
  }

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
  /// Only a queued task gets a run; the task is not (any more). Nothing was recorded.
  #[error("task {0} is not queued")]
  TaskNotQueued(TaskId),
  /// The task was edited since the revision its run was confirmed on. Nothing was recorded.
  #[error("task {0} changed since its run was confirmed")]
  TaskChangedMeanwhile(TaskId),
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
  /// The events would continue run `0`, whose worktrees a person removed: it never continues.
  /// Nothing was recorded.
  #[error("run {0} cannot continue: its worktree was removed")]
  WorktreeRemoved(RunId),
}

impl From<rusqlite::Error> for RunStoreError {
  fn from(error: rusqlite::Error) -> Self {
    Self::Store(StoreError::Sqlite(error))
  }
}
