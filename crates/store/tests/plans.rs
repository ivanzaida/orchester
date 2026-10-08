//! Plans of gated steps: proposed and decided with the run events that record it, read back with
//! their decisions and who made them, and never rewritten. A database of the schema before who decided
//! was kept reads its decisions as the person's.
//! `cargo test -p orchester-store --test plans plans`.

mod support;

use std::path::Path;

use orchester_core::{
  id::RunId,
  run::{Answerer, Plan, PlanDecision, PlanReview, ReviewNote, Run, RunEventKind, StepStatus, WaitReason},
  store::RunStore,
};
use orchester_store::{ProjectDb, RunStoreError};
use rusqlite::Connection;

use self::support::{at, copied_at_version, started_gated_run};

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

fn decide(db: &mut ProjectDb, run: &mut Run, plan: &mut Plan, (decision, by): (PlanDecision, Answerer), now: i64) {
  let events = run.decide_plan(plan, decision, by, at(now)).expect("decided");
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
  decide(
    &mut db,
    &mut run,
    &mut first,
    (changes("Keep email sign-in."), Answerer::Person),
    10,
  );
  let mut second = propose(&mut db, &mut run, "1. Add the provider.\n2. Keep email.", 11);
  assert_eq!(second.version.get(), 2);
  decide(
    &mut db,
    &mut run,
    &mut second,
    (PlanDecision::Approved, Answerer::Automation),
    12,
  );
  let plans = db.plans(run.id()).expect("plans");
  let by: Vec<_> = plans
    .iter()
    .map(|plan| plan.review.as_ref().map(|review| review.by))
    .collect();
  assert_eq!(by, [Some(Answerer::Person), Some(Answerer::Automation)]);
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
  decide(
    &mut db,
    &mut run,
    &mut plan,
    (PlanDecision::Approved, Answerer::Person),
    10,
  );
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

fn schema_version(path: &Path) -> u32 {
  Connection::open(path)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version")
}

fn keeps_who_decided(connection: &Connection) -> bool {
  connection
    .prepare("SELECT 1 FROM pragma_table_info('plans') WHERE name = 'decided_by'")
    .expect("schema")
    .exists([])
    .expect("schema")
}

/// The newest schema whose plans do not say who decided them, wherever later migrations moved the latest.
fn version_before_decided_by(latest: u32) -> u32 {
  (1..latest)
    .rev()
    .find(|&version| {
      let repo = tempfile::tempdir().expect("temporary directory");
      !keeps_who_decided(&ProjectDb::create_at_version(repo.path(), version).expect("an old database"))
    })
    .expect("a schema before decided_by")
}

#[test]
fn plans_decided_before_who_decided_was_kept_read_as_the_persons() {
  let (repo, mut db, mut run, execution) = started_gated_run();
  let events = run.start_step(execution, at(8)).expect("start");
  db.append_events(&run, &events).expect("append");
  let mut first = propose(&mut db, &mut run, "Plan.", 9);
  // Automation's, which the schema before cannot keep.
  decide(
    &mut db,
    &mut run,
    &mut first,
    (changes("Shorter."), Answerer::Automation),
    10,
  );
  let mut second = propose(&mut db, &mut run, "Shorter plan.", 11);
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let latest = schema_version(&current);
  let old = copied_at_version(&current, version_before_decided_by(latest));
  let old_database = old.path().join(".orchester/orchester.db");
  assert!(
    !keeps_who_decided(&Connection::open(&old_database).expect("open")),
    "the schema before does not say who decided"
  );

  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&old_database), latest);
  let persons = PlanReview {
    by: Answerer::Person,
    ..first.review.clone().expect("decided")
  };
  let decided = Plan {
    review: Some(persons),
    ..first
  };
  assert_eq!(upgraded.plans(run.id()).expect("plans"), [decided, second.clone()]);
  // The plan that waits takes a decision with who made it; a decided plan is still never rewritten.
  decide(
    &mut upgraded,
    &mut run,
    &mut second,
    (PlanDecision::Approved, Answerer::Automation),
    12,
  );
  let plans = upgraded.plans(run.id()).expect("plans");
  assert_eq!(
    plans[1].review.as_ref().map(|review| review.by),
    Some(Answerer::Automation)
  );
  let raw = Connection::open(&old_database).expect("open raw database");
  assert!(
    raw
      .execute("UPDATE plans SET decided_by = 'automation' WHERE version = 1", [])
      .is_err()
  );
}
