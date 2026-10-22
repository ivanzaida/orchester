//! Writes that meet a busy database (ORC-74): a write waits for a lock other connections hold longer than the five
//! seconds SQLite waited before, and a write of the run's log that fails because the database is busy, as
//! `SQLITE_BUSY_SNAPSHOT` or `SQLITE_BUSY`, starts again from the start and is stored once. Run with `cargo test -p
//! orchester-store --test busy_writes busy_writes`.

mod support;

use std::{
  thread,
  time::{Duration, Instant},
};

use orchester_core::{connection::stream::AgentEvent, store::ActivityStore, time::Timestamp};
use rusqlite::ffi;

use self::support::{model, started_run};

/// Longer than the five seconds a connection waited for a lock before.
const HELD: Duration = Duration::from_secs(6);

#[test]
fn busy_writes_a_write_waits_for_a_lock_held_longer_than_five_seconds() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), Timestamp::now())
    .expect("invocation");
  let holder = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  holder.execute_batch("BEGIN IMMEDIATE").expect("hold the write lock");
  let releasing = thread::spawn(move || {
    thread::sleep(HELD);
    holder.execute_batch("ROLLBACK").expect("let go of the lock");
  });
  let started = Instant::now();
  let sequence = db
    .append_activity(&invocation, &[(Timestamp::now(), AgentEvent::TurnStarted)])
    .expect("the write waits for the lock");
  assert!(
    started.elapsed() >= HELD - Duration::from_millis(100),
    "{:?}",
    started.elapsed()
  );
  assert_eq!(sequence, 1);
  releasing.join().expect("the holder lets go");
}

#[test]
fn busy_writes_a_busy_snapshot_and_a_busy_lock_start_the_write_again() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), Timestamp::now())
    .expect("invocation");
  let events = [
    (Timestamp::now(), AgentEvent::TurnStarted),
    (Timestamp::now(), AgentEvent::Message { text: "hello".into() }),
  ];
  orchester_store::fail_next_writes_for_test(&[ffi::SQLITE_BUSY_SNAPSHOT, ffi::SQLITE_BUSY]);
  let sequence = db
    .append_activity(&invocation, &events)
    .expect("the write succeeds on its third attempt");
  assert_eq!(sequence, 2);
  let stored = db.activity(run.id(), 0).expect("transcript");
  assert_eq!(
    stored.into_iter().map(|entry| entry.event).collect::<Vec<_>>(),
    events.map(|(_, event)| event),
    "the attempts that failed left nothing behind"
  );
}

#[test]
fn busy_writes_a_real_error_is_not_retried() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), Timestamp::now())
    .expect("invocation");
  orchester_store::fail_next_writes_for_test(&[ffi::SQLITE_FULL]);
  let started = Instant::now();
  let error = db
    .append_activity(&invocation, &[(Timestamp::now(), AgentEvent::TurnStarted)])
    .expect_err("a full disk is not waited out");
  assert!(started.elapsed() < Duration::from_secs(1), "{:?}", started.elapsed());
  assert!(!error.is_locked(), "{error}");
  assert!(db.activity(run.id(), 0).expect("transcript").is_empty());
}
