//! The run log's records (`RunLogRecord`), the values the runner's executor sends to the coordinator's sink: each
//! kind survives serialization unchanged and, written as it arrived, stores what appending the events directly
//! stores. `cargo test -p orchester-store --test run_log_records run_log_record`.

mod support;

use orchester_core::{
  run::{Answerer, InterruptCause, PlanDecision, Run, RunnerFailure},
  store::RunStore,
};
use orchester_store::{ProjectDb, RunLogRecord, RunStoreError};

use self::support::{at, started_gated_run, started_run};

/// `record` sent as JSON and read back, as a sink in another process would receive it.
fn sent(record: &RunLogRecord) -> RunLogRecord {
  let text = serde_json::to_string(record).expect("a record serializes");
  let received: RunLogRecord = serde_json::from_str(&text).expect("a record deserializes");
  assert_eq!(&received, record, "the record survives being sent");
  received
}

fn stored(db: &ProjectDb, run: &Run) -> Run {
  db.run(run.id()).expect("read").expect("the run")
}

#[test]
fn run_log_record_of_events_round_trips_and_stores_the_run() {
  let (_repo, mut db, mut run, execution) = started_run();
  let events = run.start_step(execution, at(8)).expect("started");
  db.write_run_log(&sent(&RunLogRecord::events(&run, &events).expect("a record")))
    .expect("written");
  let events = run.pause(at(9)).expect("paused");
  db.write_run_log(&sent(&RunLogRecord::events(&run, &events).expect("a record")))
    .expect("written");
  assert_eq!(stored(&db, &run), run);
  assert_eq!(Run::replay(&db.events(run.id()).expect("events")), Ok(run));
}

#[test]
fn run_log_record_with_a_plan_round_trips_and_stores_the_plan() {
  let (_repo, mut db, mut run, execution) = started_gated_run();
  let events = run.start_step(execution, at(8)).expect("started");
  db.append_events(&run, &events).expect("appended");
  let (events, mut plan) = run
    .propose_plan(execution, "1. Add the provider.".into(), &[], at(9))
    .expect("proposed");
  db.write_run_log(&sent(&RunLogRecord::with_plan(&run, &events, &plan).expect("a record")))
    .expect("written");
  let events = run
    .decide_plan(&mut plan, PlanDecision::Approved, Answerer::Automation, at(10))
    .expect("decided");
  db.write_run_log(&sent(&RunLogRecord::with_plan(&run, &events, &plan).expect("a record")))
    .expect("written");
  assert_eq!(db.plans(run.id()).expect("plans"), [plan]);
  assert_eq!(stored(&db, &run), run);
}

#[test]
fn run_log_record_of_a_runner_failure_round_trips_and_keeps_why() {
  let (_repo, mut db, mut run, _execution) = started_run();
  let events = run.interrupt(InterruptCause::RunnerFailed, at(8)).expect("interrupted");
  let record = RunLogRecord::runner_failure(&run, &events, RunnerFailure::StorageContention).expect("a record");
  db.write_run_log(&sent(&record)).expect("written");
  assert_eq!(stored(&db, &run), run);
  assert_eq!(
    db.runner_failure(run.id()).expect("read"),
    Some(RunnerFailure::StorageContention)
  );
}

#[test]
fn run_log_record_keeps_the_stores_checks_after_it_was_sent() {
  let (_repo, mut db, mut run, execution) = started_run();
  let events = run.start_step(execution, at(8)).expect("started");
  let record = sent(&RunLogRecord::events(&run, &events).expect("a record"));
  db.write_run_log(&record).expect("written");
  assert!(
    matches!(db.write_run_log(&record), Err(RunStoreError::SequenceConflict { .. })),
    "the same events again do not follow the stored log"
  );
  let events = run.interrupt(InterruptCause::Crashed, at(9)).expect("interrupted");
  let record = RunLogRecord::runner_failure(&run, &events, RunnerFailure::Other).expect("a record");
  assert!(
    matches!(db.write_run_log(&sent(&record)), Err(RunStoreError::EventsMismatch(_))),
    "only the runner's own failure is kept with an interruption"
  );
  let empty = RunLogRecord::events(&run, &[]).expect("a record");
  assert!(matches!(
    db.write_run_log(&sent(&empty)),
    Err(RunStoreError::EventsMismatch(_))
  ));
}
