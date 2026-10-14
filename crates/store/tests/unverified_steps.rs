//! Steps that wait for a person unverified or unchanged, what an unverified output could not check, the revision a
//! failed execution was for, and the migration that added them.

mod support;

use orchester_core::{
  resource::StepKey,
  revision::Revision,
  run::{Run, RunStatus, StepFailure, StepOutcome, StepOutput, Stuck, StuckDecision, UnverifiedCheck},
  store::{ActivityStore, RunStore},
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{at, copied_at_version, started_run};

/// The schema version before steps could be unverified.
const BEFORE_UNVERIFIED: u32 = 59;

fn implement() -> StepKey {
  StepKey::new("implement").expect("key")
}

fn revision(digit: char) -> Revision {
  Revision::new(&digit.to_string().repeat(40)).expect("revision")
}

fn reloaded(db: &ProjectDb, run: &Run) -> Run {
  db.run(run.id()).expect("read").expect("run")
}

/// Starts the run's current execution of `implement` and fails it unverified on revision `b`.
fn unverified(db: &mut ProjectDb, run: &mut Run, millis: i64) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, at(millis)).expect("started");
  db.append_events(run, &events).expect("stored start");
  let events = run
    .fail_judged(
      execution,
      (StepFailure::Unverified, Some(revision('b'))),
      at(millis + 1),
    )
    .expect("unverified");
  db.append_events(run, &events).expect("stored failure");
}

#[test]
fn unverified_steps_round_trip_waiting_retried_unchanged_and_accepted() {
  let (_repo, mut db, mut run, _) = started_run();
  unverified(&mut db, &mut run, 10);
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::Unverified(implement()))
  );
  let stored = reloaded(&db, &run);
  assert_eq!(stored, run);
  assert_eq!(
    stored
      .latest_execution(&implement())
      .and_then(|execution| execution.judged()),
    Some(&revision('b'))
  );

  // The person runs the step again; it is not run on the same revision without them.
  let events = run.decide_stuck(&StuckDecision::Retry, at(20)).expect("retried");
  db.append_events(&run, &events).expect("stored retry");
  let again = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.refuse_unchanged(again, revision('b'), at(21)).expect("refused");
  db.append_events(&run, &events).expect("stored refusal");
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::Unchanged(implement()))
  );
  assert_eq!(reloaded(&db, &run), run);

  let decision = StuckDecision::Accept {
    by: orchester_core::run::Answerer::Person,
  };
  let events = run.decide_stuck(&decision, at(30)).expect("accepted");
  db.append_events(&run, &events).expect("stored acceptance");
  // The acceptance's attempt, which the runner passes the step in, round trips.
  assert!(matches!(
    run.current_attempt().map(|attempt| attempt.reason()),
    Some(orchester_core::run::AttemptReason::Accepted { .. })
  ));
  assert_eq!(reloaded(&db, &run), run);
  let accepted = run.current_attempt().expect("attempt").executions()[0].id();
  let mut events = run.start_step(accepted, at(31)).expect("started");
  events.extend(run.complete_step(accepted, at(32)).expect("passed"));
  db.append_events(&run, &events).expect("stored pass");
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(reloaded(&db, &run), run);
  assert_eq!(Run::replay(&db.events(run.id()).expect("events")), Ok(run));
}

#[test]
fn unverified_steps_output_keeps_what_could_not_be_checked() {
  let (_repo, mut db, run, execution) = started_run();
  let output = StepOutput {
    run: run.id(),
    execution,
    step: implement(),
    outcome: Some(StepOutcome::Unverified),
    text: "Two states could not be reached.".into(),
    unverified: vec![UnverifiedCheck {
      check: "Summary 02.B".into(),
      reason: "no fixture reaches it".into(),
    }],
    created_at: at(10),
  };
  db.save_step_output(&output).expect("save");
  assert_eq!(db.step_outputs(run.id()).expect("outputs"), [output]);
}

#[test]
fn unverified_steps_migration_keeps_executions_and_outputs() {
  let (repo, mut db, run, execution) = started_run();
  let output = StepOutput {
    run: run.id(),
    execution,
    step: implement(),
    outcome: Some(StepOutcome::Done),
    text: "Done.".into(),
    unverified: Vec::new(),
    created_at: at(10),
  };
  db.save_step_output(&output).expect("save");
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_UNVERIFIED);
  let path = old.path().join(".orchester").join("orchester.db");
  let old_schema = Connection::open(&path).expect("open the old database");
  // The old schema knows no unverified failure.
  assert!(
    old_schema
      .execute(
        "UPDATE step_executions SET status = 'failed', failure = 'unverified' WHERE id = ?1",
        [execution.to_string()],
      )
      .is_err()
  );
  drop(old_schema);
  let db = ProjectDb::open(old.path()).expect("migrate existing project");
  assert_eq!(reloaded(&db, &run), run);
  assert_eq!(db.step_outputs(run.id()).expect("outputs"), [output]);
}
