//! The local sink: the executor's records written to the project database beside it, with the store's own functions
//! and transactions, and their busy retry (`docs/architecture/storage.md`, "Concurrency"). Each record is written as
//! it comes, so the transcript's entries reach the database, and the desktop's `AgentActivity`, in the batches the
//! agent's worker flushes, as before the sink.

use std::path::{Path, PathBuf};

use orchester_store::{
  ActivityRecord, InvocationRecord, InvocationStart, ProjectDb, RunLogRecord, RunStoreError, StoreError,
};

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

impl Sink for LocalSink {
  fn run_log(&mut self, record: RunLogRecord) -> Result<(), RunStoreError> {
    self.write(|db| db.write_run_log(&record))
  }

  fn start_invocation(&mut self, start: InvocationStart) -> Result<u32, StoreError> {
    let start = sent(start);
    self.write(|db| db.write_invocation_start(&start))
  }

  fn activity(&mut self, record: ActivityRecord) -> Result<u64, StoreError> {
    let record = sent(record);
    self.write(|db| db.write_activity(&record))
  }

  fn invocation(&mut self, record: InvocationRecord) -> Result<(), StoreError> {
    let record = sent(record);
    self.write(|db| db.write_invocation(&record))
  }

  fn another(&self) -> Box<dyn Sink> {
    Box::new(Self::new(&self.root))
  }
}

/// `record` as a coordinator in another process would receive it: in the runner's own tests, every invocation record
/// crosses the sink as JSON (`docs/architecture/remote-runners.md`, rule 4).
#[cfg(feature = "test-agent")]
fn sent<T>(record: T) -> T
where
  T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
  let text = serde_json::to_string(&record).expect("a record the executor sends serializes");
  let received: T = serde_json::from_str(&text).expect("a record the executor sends deserializes");
  assert_eq!(received, record, "the record survives being sent");
  received
}

/// `record`, which the local sink writes as it is.
#[cfg(not(feature = "test-agent"))]
const fn sent<T>(record: T) -> T {
  record
}
