//! The ends of the project's runs the Orchestrator chat follows: since when it follows them (set when the project's
//! database gains them, so ends before are history), and the event that put a run where it stands, which keeps each
//! end once. `cargo test -p orchester-store --test orchestrator_run_ends run_ends`.

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
fn run_ends_name_the_latest_event_of_a_kind() {
  let (_repo, mut db, mut run, _execution) = started_run();
  assert_eq!(db.latest_run_event(run.id(), "interrupted").expect("read"), None);
  let (started, when) = db
    .latest_run_event(run.id(), "started")
    .expect("read")
    .expect("it started");
  assert_eq!(when, at(7));
  let events = run.interrupt(InterruptCause::Crashed, at(9)).expect("interrupted");
  db.append_events(&run, &events).expect("append");
  let (interrupted, when) = db
    .latest_run_event(run.id(), "interrupted")
    .expect("read")
    .expect("it was interrupted");
  assert_eq!(when, at(9));
  assert!(interrupted.get() > started.get(), "the newest event of its kind");
}
