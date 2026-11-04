//! The local sink: the executor's records written to the project database beside it, with the store's own functions
//! and transactions, and their busy retry (`docs/architecture/storage.md`, "Concurrency"). Each record is written as
//! it comes, so the transcript's entries reach the database, and the desktop's `AgentActivity`, in the batches the
//! agent's worker flushes, as before the sink.

use std::path::{Path, PathBuf};

use orchester_store::{
  ActivityRecord, ApprovalRecord, InvocationRecord, InvocationStart, MessageRecord, ProjectDb, QuestionRecord,
  RunLogRecord, RunStoreError, StoreError,
};

#[cfg(feature = "test-agent")]
use crate::execution::executor::link::sent;
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

  /// Runs `write` on the sink's connection, opened first if it has none; a failed write drops it.
  fn write<T, E: From<StoreError>>(&mut self, write: impl FnOnce(&mut ProjectDb) -> Result<T, E>) -> Result<T, E> {
    let db = match &mut self.db {
      Some(db) => db,
      None => self.db.insert(ProjectDb::open(&self.root)?),
    };
    let written = write(db);
    if written.is_err() {
      self.db = None;
    }
    written
  }
}

// In the runner's own tests every invocation, conversation, question and tool request record crosses the sink as JSON
// and must arrive unchanged, as a coordinator in another process would receive it
// (`docs/architecture/remote-runners.md`, rule 4); the run log's records cross as JSON too.
impl Sink for LocalSink {
  fn run_log(&mut self, record: RunLogRecord) -> Result<(), RunStoreError> {
    #[cfg(feature = "test-agent")]
    sent::sending(&record);
    self.write(|db| db.write_run_log(&record))
  }

  fn invocation_start(&mut self, start: InvocationStart) -> Result<u32, StoreError> {
    #[cfg(feature = "test-agent")]
    sent::survives_sending(&start);
    self.write(|db| db.write_invocation_start(&start))
  }

  fn activity(&mut self, record: ActivityRecord) -> Result<u64, StoreError> {
    #[cfg(feature = "test-agent")]
    sent::survives_sending(&record);
    self.write(|db| db.write_activity(&record))
  }

  fn invocation(&mut self, record: InvocationRecord) -> Result<(), StoreError> {
    #[cfg(feature = "test-agent")]
    sent::survives_sending(&record);
    self.write(|db| db.write_invocation(&record))
  }

  fn message(&mut self, record: MessageRecord) -> Result<bool, StoreError> {
    #[cfg(feature = "test-agent")]
    sent::survives_sending(&record);
    self.write(|db| db.write_message(&record))
  }

  fn question(&mut self, record: QuestionRecord) -> Result<bool, StoreError> {
    #[cfg(feature = "test-agent")]
    sent::survives_sending(&record);
    self.write(|db| db.write_question(&record))
  }

  fn approval(&mut self, record: ApprovalRecord) -> Result<bool, StoreError> {
    #[cfg(feature = "test-agent")]
    sent::survives_sending(&record);
    self.write(|db| db.write_approval(&record))
  }

  fn another(&self) -> Box<dyn Sink> {
    Box::new(Self::new(&self.root))
  }
}
