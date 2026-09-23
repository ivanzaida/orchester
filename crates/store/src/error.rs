use std::{io, path::PathBuf};

use orchester_core::id::TaskId;
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
  /// A stored value no longer satisfies the domain rules; the database was edited or damaged.
  #[error("stored {what} is invalid: {detail}")]
  Corrupt { what: &'static str, detail: String },
  /// A value cannot be represented in the database, e.g. a non-UTF-8 path.
  #[error("{0} cannot be stored")]
  Unrepresentable(&'static str),
  #[error("task {0} does not exist")]
  TaskNotFound(TaskId),
  #[error("a task batch must contain at least one task")]
  EmptyBatch,
}

impl StoreError {
  pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
    let path = path.into();
    move |source| Self::Io { path, source }
  }
}
