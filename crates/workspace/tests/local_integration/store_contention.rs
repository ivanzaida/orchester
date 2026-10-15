//! The project database stays usable while local integration journals its Git processes: each holds the store,
//! whose SQLite sidecars other connections create and delete meanwhile, and the task page reads the task beside it.
//! Neither fails.

use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
};

use orchester_core::{integration::local::LocalProcessStore, store::RunStore};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::detail::TaskDetail;

use super::support::Fixture;

#[test]
fn local_integration_journal_writes_beside_task_reads_never_lock_the_database() {
  let fixture = Fixture::new(false);
  let root = fixture.root().to_owned();
  let run = fixture.run;
  let task = ProjectDb::open(&root)
    .expect("db")
    .run(run)
    .expect("run")
    .expect("exists")
    .task();
  drop(fixture.open());
  let writing = Arc::new(AtomicBool::new(true));
  let writer = {
    let (root, writing) = (root.clone(), writing.clone());
    thread::spawn(move || {
      let mut failures = Vec::new();
      for _ in 0..150 {
        // As each Git process of an operation: the store held, then three short journal writes.
        let result = orchester_exec::local_git::owned::StoreOwnership::acquire(&root)
          .map_err(|error| error.to_string())
          .and_then(|_owner| {
            let sequence = ProjectDb::open(&root)
              .and_then(|mut db| db.begin_local_process(run))
              .map_err(|error| error.to_string())?;
            ProjectDb::open(&root)
              .and_then(|mut db| db.local_process_started(run, sequence, None))
              .map_err(|error| error.to_string())?;
            ProjectDb::open(&root)
              .and_then(|mut db| db.finish_local_process(run, sequence))
              .map_err(|error| error.to_string())
          });
        if let Err(error) = result {
          failures.push(error);
        }
      }
      writing.store(false, Ordering::Release);
      failures
    })
  };
  let mut failures = Vec::new();
  let mut reads = 0;
  while writing.load(Ordering::Acquire) {
    if let Err(error) = TaskDetail::load_in(&root, task) {
      failures.push(error.to_string());
    }
    reads += 1;
  }
  let written = writer.join().expect("writer");
  assert!(reads > 0, "the reader ran beside the writer");
  assert!(written.is_empty(), "journal writes failed: {written:?}");
  assert!(failures.is_empty(), "task reads failed after {reads}: {failures:?}");
}
