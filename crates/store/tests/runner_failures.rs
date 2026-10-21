//! Why the runner could not record or carry on a run it interrupted (ORC-75): the one place that reads it from SQLite
//! (busy or locked is transient contention; damaged, full, read-only or another schema is storage unavailable; the
//! rest is another failure), and the kind kept with the interruption in its transaction, read back for the run's last
//! interruption and counted by run for the project's brake. `cargo test -p orchester-store --test runner_failures
//! runner_failure`.

mod support;

use orchester_core::{
  run::{InterruptCause, RunnerFailure},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::{RunStoreError, StoreError};
use rusqlite::{Connection, ffi};
use support::{at, started_run};
use tempfile::TempDir;

fn sqlite(code: i32) -> StoreError {
  StoreError::Sqlite(rusqlite::Error::SqliteFailure(ffi::Error::new(code), None))
}

#[test]
fn runner_failure_of_a_busy_or_locked_database_is_contention() {
  for code in [
    ffi::SQLITE_BUSY,
    ffi::SQLITE_BUSY_SNAPSHOT,
    ffi::SQLITE_BUSY_TIMEOUT,
    ffi::SQLITE_LOCKED,
    ffi::SQLITE_LOCKED_SHAREDCACHE,
  ] {
    assert_eq!(
      sqlite(code).runner_failure(),
      RunnerFailure::StorageContention,
      "{code}"
    );
  }
  // What a writer meets while another connection holds the write lock: "database is locked".
  let folder = TempDir::new().expect("a folder");
  let path = folder.path().join("locked.db");
  let holder = Connection::open(&path).expect("open");
  holder.execute_batch("CREATE TABLE t (x INTEGER); BEGIN IMMEDIATE; INSERT INTO t VALUES (1);").expect("held");
  let writer = Connection::open(&path).expect("open");
  writer.busy_timeout(std::time::Duration::ZERO).expect("no wait");
  let locked = StoreError::from(writer.execute("INSERT INTO t VALUES (2)", []).expect_err("locked"));
  assert!(locked.to_string().contains("database is locked"), "{locked}");
  assert_eq!(locked.runner_failure(), RunnerFailure::StorageContention);
  assert_eq!(
    RunStoreError::Store(locked).runner_failure(),
    RunnerFailure::StorageContention,
    "beneath the run store's error too"
  );
}

#[test]
fn runner_failure_of_storage_unavailable_for_real_and_of_the_rest() {
  for code in [
    ffi::SQLITE_CORRUPT,
    ffi::SQLITE_NOTADB,
    ffi::SQLITE_FULL,
    ffi::SQLITE_IOERR,
    ffi::SQLITE_READONLY,
    ffi::SQLITE_CANTOPEN,
    ffi::SQLITE_PERM,
  ] {
    assert_eq!(sqlite(code).runner_failure(), RunnerFailure::StorageFailure, "{code}");
  }
  let schema = StoreError::SchemaTooNew {
    found: 9,
    supported: 8,
  };
  assert_eq!(schema.runner_failure(), RunnerFailure::StorageFailure, "another schema");
  let damaged = StoreError::Corrupt {
    what: "run",
    detail: "x".to_owned(),
  };
  assert_eq!(damaged.runner_failure(), RunnerFailure::StorageFailure);
  assert_eq!(sqlite(ffi::SQLITE_CONSTRAINT).runner_failure(), RunnerFailure::Other);
  assert_eq!(StoreError::EmptyBatch.runner_failure(), RunnerFailure::Other);
  assert_eq!(
    RunStoreError::EventsMismatch("x").runner_failure(),
    RunnerFailure::Other,
    "a rule of the run's log"
  );
}

#[test]
fn runner_failure_is_kept_with_its_interruption() {
  let (_repo, mut db, mut run, _execution) = started_run();
  assert_eq!(db.runner_failure(run.id()).expect("read"), None, "never interrupted");
  let events = run.interrupt(InterruptCause::Crashed, at(1)).expect("interrupted");
  assert!(
    matches!(
      db.append_runner_failure(&run, &events, RunnerFailure::Other),
      Err(RunStoreError::EventsMismatch(_))
    ),
    "only the runner's own failure has a kind"
  );
  db.append_events(&run, &events).expect("recorded");
  assert_eq!(db.runner_failure(run.id()).expect("read"), None, "a crash");
  let events = run.continue_run(at(2)).expect("continued");
  db.append_events(&run, &events).expect("recorded");
  let interrupted = Timestamp::now();
  let events = run.interrupt(InterruptCause::RunnerFailed, interrupted).expect("interrupted");
  db.append_runner_failure(&run, &events, RunnerFailure::StorageContention)
    .expect("recorded with why");
  assert_eq!(
    db.run(run.id()).expect("read").expect("it").status(),
    run.status(),
    "the interruption is stored"
  );
  assert_eq!(
    db.runner_failure(run.id()).expect("read"),
    Some(RunnerFailure::StorageContention)
  );
  assert_eq!(db.contended_runs_since(at(0)).expect("count"), 1);
  assert_eq!(
    db.contended_runs_since(Timestamp::from_unix_millis(interrupted.unix_millis() + 1))
      .expect("count"),
    0,
    "an interruption before since is not counted"
  );
  // Continued and interrupted so again: still one run of the project.
  let events = run.continue_run(Timestamp::now()).expect("continued");
  db.append_events(&run, &events).expect("recorded");
  let events = run
    .interrupt(InterruptCause::RunnerFailed, Timestamp::now())
    .expect("interrupted");
  db.append_runner_failure(&run, &events, RunnerFailure::StorageContention)
    .expect("recorded with why");
  assert_eq!(db.contended_runs_since(at(0)).expect("count"), 1, "counted by run");
  // Its last interruption decides: a crash after it has no kind.
  let events = run.continue_run(Timestamp::now()).expect("continued");
  db.append_events(&run, &events).expect("recorded");
  let events = run.interrupt(InterruptCause::Crashed, Timestamp::now()).expect("interrupted");
  db.append_events(&run, &events).expect("recorded");
  assert_eq!(db.runner_failure(run.id()).expect("read"), None);
}
