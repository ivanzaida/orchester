//! The local sink: the executor's records written to the project database beside it, with the store's own functions
//! and transactions, and their busy retry (`docs/architecture/storage.md`, "Concurrency").

use std::path::{Path, PathBuf};

use orchester_store::{ProjectDb, RunLogRecord, RunStoreError};

use crate::execution::executor::sink::Sink;

/// Writes through a connection of its own, opened at the first record. A write that fails drops it, so the next one,
/// such as the runner's interruption of the run after that failure, goes through a fresh connection.
pub(in crate::execution) struct LocalSink {
  root: PathBuf,
  db: Option<ProjectDb>,
}

impl LocalSink {
  pub(in crate::execution) fn new(root: &Path) -> Self {
    Self {
      root: root.to_path_buf(),
      db: None,
    }
  }
}

impl Sink for LocalSink {
  fn run_log(&mut self, record: RunLogRecord) -> Result<(), RunStoreError> {
    let db = match &mut self.db {
      Some(db) => db,
      None => self.db.insert(ProjectDb::open(&self.root)?),
    };
    let written = db.write_run_log(&record);
    if written.is_err() {
      self.db = None;
    }
    written
  }
}
