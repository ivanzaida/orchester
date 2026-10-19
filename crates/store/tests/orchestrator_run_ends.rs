//! The ends of the project's runs the Orchestrator chat follows: since when it follows them (set when the project's
//! database gains them, so ends before are history), and the tasks' latest runs by the event that ended or stopped
//! them, which keeps each end once. `cargo test -p orchester-store --test orchestrator_run_ends run_ends`.

mod support;

use orchester_core::{run::InterruptCause, store::RunStore, time::Timestamp};
use support::{at, started_run};

#[test]
fn run_ends_are_followed_from_when_the_database_gained_them() {
  let before = Timestamp::now();
  let (_repo, db, _run, _execution) = started_run();
  let since = db.run_ends_since().expect("read");
  assert!(since >= Timestamp::from_unix_millis(before.unix_millis() - 1_000) && since <= Timestamp::now());
}

#[test]
fn run_ends_read_the_latest_run_by_the_event_that_ended_or_stopped_it() {
  let (_repo, mut db, mut run, _execution) = started_run();
  let since = Timestamp::from_unix_millis(0);
  assert_eq!(db.ended_runs_since(since).expect("read"), Vec::new(), "at work");
  let events = run.interrupt(InterruptCause::Crashed, at(9)).expect("interrupted");
  db.append_events(&run, &events).expect("append");
  let ended = db.ended_runs_since(since).expect("read");
  assert_eq!(ended.len(), 1, "{ended:?}");
  assert_eq!(
    (ended[0].run, ended[0].kind.as_str(), ended[0].at),
    (run.id(), "interrupted", at(9))
  );
  assert_eq!(ended[0].sequence, run.last_sequence());
  assert!(ended[0].after.is_some(), "the event before it");
  assert_eq!(
    db.ended_runs_since(at(10)).expect("read"),
    Vec::new(),
    "an end before since is not read"
  );
}
