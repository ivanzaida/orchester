//! Writers in parallel on one project database (ORC-76): several connections writing the run's transcript at once,
//! as a run's agents and the app do, while another connection holds the write lock longer than the five seconds a
//! write waited before; none of them fails and every line is stored once. And readers that keep their connection
//! open between reads, as the Orchestrator chat does, hold no read open that keeps a checkpoint from emptying the
//! write-ahead log. Run with `cargo test -p orchester-store --test parallel_writers parallel_writers`.

mod support;

use std::{
  path::{Path, PathBuf},
  sync::{
    Arc, Barrier,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use orchester_core::{connection::stream::AgentEvent, store::ActivityStore, time::Timestamp};
use orchester_store::ProjectDb;

use self::support::{model, started_run};

/// How many connections write at once.
const WRITERS: usize = 6;
/// How many transcript writes each of them makes.
const WRITES: usize = 40;
/// Longer than the five seconds a connection waited for a lock before.
const SLOW_WRITE: Duration = Duration::from_secs(6);

fn database(repo: &Path) -> PathBuf {
  repo.join(".orchester").join("orchester.db")
}

#[test]
fn parallel_writers_outlast_a_slow_writer_and_store_every_line_once() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), Timestamp::now())
    .expect("invocation");
  let start = Arc::new(Barrier::new(WRITERS + 1));
  let writers: Vec<_> = (0..WRITERS)
    .map(|writer| {
      let (root, invocation, start) = (repo.path().to_path_buf(), invocation.clone(), Arc::clone(&start));
      thread::spawn(move || {
        let mut db = ProjectDb::open(&root).map_err(|error| format!("writer {writer} open: {error}"))?;
        start.wait();
        for write in 0..WRITES {
          let line = AgentEvent::Message {
            text: format!("writer {writer} line {write}"),
          };
          db.append_activity(&invocation, &[(Timestamp::now(), line)])
            .map_err(|error| format!("writer {writer} line {write}: {error}"))?;
          thread::sleep(Duration::from_millis(5));
        }
        Ok::<_, String>(())
      })
    })
    .collect();
  let slow = rusqlite::Connection::open(database(repo.path())).expect("open");
  start.wait();
  thread::sleep(Duration::from_millis(50));
  slow
    .execute_batch("BEGIN IMMEDIATE; UPDATE project SET name = 'Kontur Two';")
    .expect("hold the write lock");
  let held = Instant::now();
  thread::sleep(SLOW_WRITE);
  slow.execute_batch("COMMIT").expect("end the slow write");
  let slow_for = held.elapsed();
  for writer in writers {
    writer
      .join()
      .expect("the writer does not panic")
      .expect("no write fails");
  }
  assert!(slow_for >= SLOW_WRITE, "{slow_for:?}");
  let mut lines: Vec<String> = db
    .activity(run.id(), 0)
    .expect("transcript")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::Message { text } => Some(text),
      _ => None,
    })
    .collect();
  lines.sort();
  let mut expected: Vec<String> = (0..WRITERS)
    .flat_map(|writer| (0..WRITES).map(move |write| format!("writer {writer} line {write}")))
    .collect();
  expected.sort();
  assert_eq!(lines, expected, "every line is stored once");
}

#[test]
fn parallel_writers_readers_that_keep_their_connection_hold_no_checkpoint_back() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), Timestamp::now())
    .expect("invocation");
  let stop = Arc::new(AtomicBool::new(false));
  // The Orchestrator chat: one connection kept between its reads.
  let reader = {
    let (root, stop) = (repo.path().to_path_buf(), Arc::clone(&stop));
    let run = run.id();
    thread::spawn(move || {
      let db = ProjectDb::open(&root).expect("the reader's connection");
      let mut reads = 0_u64;
      while !stop.load(Ordering::Acquire) {
        db.activity(run, 0).expect("the reader reads");
        reads += 1;
        thread::sleep(Duration::from_millis(2));
      }
      (db, reads)
    })
  };
  for write in 0..(WRITERS * WRITES) {
    let line = AgentEvent::Message {
      text: format!("line {write}"),
    };
    db.append_activity(&invocation, &[(Timestamp::now(), line)])
      .expect("the write is stored");
  }
  stop.store(true, Ordering::Release);
  let (kept, reads) = reader.join().expect("the reader does not panic");
  assert!(reads > 0, "the reader read beside the writes");

  // The reader's connection stays open, idle between reads, as the chat's does: a checkpoint still copies the
  // whole log and empties it.
  let checkpointer = rusqlite::Connection::open(database(repo.path())).expect("open");
  let (busy, log, copied): (i64, i64, i64) = checkpointer
    .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
      Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })
    .expect("checkpoint");
  assert_eq!((busy, log, copied), (0, 0, 0), "no open read holds the checkpoint back");
  let wal = std::fs::metadata(repo.path().join(".orchester").join("orchester.db-wal")).expect("the log");
  assert_eq!(wal.len(), 0, "the log is emptied");
  drop(kept);
}
