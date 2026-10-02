//! Runs that wait for a person after their retries ran out or their step broke again, the attempts a
//! person's decision starts, and the migration that added them: runs that failed before stay failed.

mod support;

use orchester_core::{
  resource::StepKey,
  run::{AttemptReason, ReviewNote, Run, RunFailure, RunStatus, StepFailure, Stuck, StuckDecision},
  store::RunStore,
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{at, copied_at_version, started_run};

/// The schema version before runs could wait for a person.
const BEFORE_STUCK_RUNS: u32 = 33;

fn implement() -> StepKey {
  StepKey::new("implement").expect("key")
}

fn reloaded(db: &ProjectDb, run: &Run) -> Run {
  db.run(run.id()).expect("read").expect("run")
}

/// Starts the run's current execution of `implement` and fails it for `reason`.
fn fail(db: &mut ProjectDb, run: &mut Run, reason: StepFailure, millis: i64) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, at(millis)).expect("started");
  db.append_events(run, &events).expect("stored start");
  let events = run.fail_step(execution, reason, at(millis + 1)).expect("failed");
  db.append_events(run, &events).expect("stored failure");
}

#[test]
fn stuck_runs_round_trip_waiting_restarted_and_returned() {
  let (_repo, mut db, mut run, _) = started_run();
  fail(&mut db, &mut run, StepFailure::AgentFailed, 10);
  assert_eq!(
    reloaded(&db, &run)
      .current_attempt()
      .map(|attempt| attempt.reason().clone()),
    Some(AttemptReason::Restart { step: implement() })
  );
  fail(&mut db, &mut run, StepFailure::AgentFailed, 12);
  assert!(matches!(
    run.status(),
    RunStatus::WaitingForPerson(Stuck::BrokeAgain(_))
  ));
  assert_eq!(reloaded(&db, &run), run);

  let note = ReviewNote::new("Start from the failing test.").expect("note");
  let decision = StuckDecision::ReturnTo {
    step: implement(),
    note: note.clone(),
  };
  let events = run.decide_stuck(&decision, at(20)).expect("returned");
  db.append_events(&run, &events).expect("stored decision");
  let stored = reloaded(&db, &run);
  assert_eq!(stored, run);
  assert!(matches!(
    stored.current_attempt().map(|attempt| attempt.reason()),
    Some(AttemptReason::Returned { note: kept, .. }) if *kept == note
  ));

  fail(&mut db, &mut run, StepFailure::AgentFailed, 30);
  fail(&mut db, &mut run, StepFailure::AgentFailed, 32);
  let events = run.decide_stuck(&StuckDecision::Stop, at(40)).expect("stopped");
  db.append_events(&run, &events).expect("stored stop");
  assert!(matches!(run.status(), RunStatus::Failed(RunFailure::StepFailed(_))));
  assert_eq!(reloaded(&db, &run), run);
  assert_eq!(Run::replay(&db.events(run.id()).expect("events")), Ok(run));
}

#[test]
fn stuck_runs_migration_keeps_runs_that_failed_after_their_retries_failed() {
  let (repo, mut db, mut run, _) = started_run();
  fail(&mut db, &mut run, StepFailure::ReportedFailed, 10);
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");
  // As versions before runs waited for a person recorded a run whose retries ran out.
  Connection::open(&current)
    .expect("open")
    .execute(
      "UPDATE runs SET status_reason = 'retries_exhausted' WHERE id = ?1",
      [run.id().to_string()],
    )
    .expect("an earlier failure");
  let old = copied_at_version(&current, BEFORE_STUCK_RUNS);
  let db = ProjectDb::open(old.path()).expect("migrate existing project");
  let migrated = reloaded(&db, &run);
  assert_eq!(
    migrated.status(),
    &RunStatus::Failed(RunFailure::RetriesExhausted(implement()))
  );
  assert_eq!(migrated.attempts(), run.attempts());
}
