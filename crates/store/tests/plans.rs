//! Plans of gated steps: proposed and decided with the run events that record it, read back with
//! their decisions, and never rewritten.

mod support;

use orchester_core::{
  id::RunId,
  run::{Plan, PlanDecision, ReviewNote, Run, RunEventKind, StepStatus, WaitReason},
  store::RunStore,
};
use orchester_store::{ProjectDb, RunStoreError};
use rusqlite::Connection;

use self::support::{at, started_gated_run};

fn changes(text: &str) -> PlanDecision {
  PlanDecision::ChangesRequested {
    note: ReviewNote::new(text).expect("a note"),
  }
}

fn propose(db: &mut ProjectDb, run: &mut Run, text: &str, now: i64) -> Plan {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let earlier = db.plans(run.id()).expect("plans");
  let (events, plan) = run
    .propose_plan(execution, text.to_owned(), &earlier, at(now))
    .expect("proposed");
  db.append_events_with_plan(run, &events, &plan).expect("stored");
  plan
}

fn decide(db: &mut ProjectDb, run: &mut Run, plan: &mut Plan, decision: PlanDecision, now: i64) {
  let events = run.decide_plan(plan, decision, at(now)).expect("decided");
  db.append_events_with_plan(run, &events, plan).expect("stored");
}

#[test]
fn plans_round_trip_with_their_decisions() {
  let (_repo, mut db, mut run, execution) = started_gated_run();
  let events = run.start_step(execution, at(8)).expect("start");
  db.append_events(&run, &events).expect("append");
  let mut first = propose(&mut db, &mut run, "1. Add the provider.", 9);
  assert_eq!(db.plans(run.id()).expect("plans"), [first.clone()]);
  let stored = db.run(run.id()).expect("read").expect("run");
  let waiting = stored.latest_execution(&first.step).map(|execution| execution.status());
  assert_eq!(waiting, Some(StepStatus::Waiting(WaitReason::Plan)));
  decide(&mut db, &mut run, &mut first, changes("Keep email sign-in."), 10);
  let mut second = propose(&mut db, &mut run, "1. Add the provider.\n2. Keep email.", 11);
  assert_eq!(second.version.get(), 2);
  decide(&mut db, &mut run, &mut second, PlanDecision::Approved, 12);
  let plans = db.plans(run.id()).expect("plans");
  assert_eq!(plans, [first, second]);
  let events = db.events(run.id()).expect("events");
  let requested = events.iter().any(|event| {
    matches!(
      event.kind(),
      RunEventKind::PlanDecided { decision: PlanDecision::ChangesRequested { note }, .. }
        if note.as_str() == "Keep email sign-in."
    )
  });
  assert!(requested);
  // The stored state is what the stored events describe.
  assert_eq!(Run::replay(&events), Ok(db.run(run.id()).expect("read").expect("run")));
}

#[test]
fn plans_keep_their_text_and_are_decided_once() {
  let (repo, mut db, mut run, execution) = started_gated_run();
  let events = run.start_step(execution, at(8)).expect("start");
  db.append_events(&run, &events).expect("append");
  let mut plan = propose(&mut db, &mut run, "Plan.", 9);
  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  assert!(raw.execute("UPDATE plans SET text = 'Other.'", []).is_err());
  decide(&mut db, &mut run, &mut plan, PlanDecision::Approved, 10);
  assert!(
    raw
      .execute("UPDATE plans SET decision = 'changes_requested', note = 'No.'", [])
      .is_err()
  );
  assert!(raw.execute("DELETE FROM plans", []).is_err());
  assert_eq!(db.plans(run.id()).expect("plans"), [plan]);
}

#[test]
fn plans_are_stored_only_with_their_own_run() {
  let (_repo, mut db, mut run, execution) = started_gated_run();
  let events = run.start_step(execution, at(8)).expect("start");
  db.append_events(&run, &events).expect("append");
  let (events, mut plan) = run
    .propose_plan(execution, "Plan.".into(), &[], at(9))
    .expect("proposed");
  plan.run = RunId::generate();
  assert!(matches!(
    db.append_events_with_plan(&run, &events, &plan),
    Err(RunStoreError::EventsMismatch(_))
  ));
  assert!(db.plans(run.id()).expect("plans").is_empty());
}
