//! Deleting a task (ORC-91): its rows go with every row that refers to them, another task and its run stay
//! whole, the schema's guards that refuse deleting history are back once the deletion committed, and a task whose run
//! has not ended is not deleted.
//!
//! `cargo test -p orchester-store --test task_deletion task_deletion_`

mod support;

use orchester_core::{
  id::IdempotencyKey,
  run::NewRun,
  store::{RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
};
use orchester_store::StoreError;
use support::{at, name, started_run};

#[test]
fn task_deletion_deletes_its_rows_and_keeps_other_tasks_and_the_guards() {
  let (repo, mut db, mut run, _) = started_run();
  let details = TaskDetails {
    title: name("Say goodbye"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let other_task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(8))
    .expect("task")[0]
    .id();
  let new = NewRun {
    task: other_task,
    snapshot: run.snapshot().clone(),
    branch: orchester_core::run::BranchName::new("orchester/orc-2-run-1").expect("branch"),
    base: run.base().clone(),
    base_branch: None,
    budget: None,
    max_duration: None,
  };
  let mut other = db.create_run(IdempotencyKey::generate(), new, at(9)).expect("run");
  let events = other.start(at(10)).expect("start");
  db.append_events(&other, &events).expect("append");

  let refused = db.delete_task(run.task(), at(11));
  assert!(
    matches!(refused, Err(StoreError::TaskRunActive(active)) if active == run.id()),
    "{refused:?}"
  );
  let events = run.cancel(at(12)).expect("cancel");
  db.append_events(&run, &events).expect("append");
  assert_eq!(db.task_deletion_facts(run.task()).expect("facts").runs, 1);

  db.delete_task(run.task(), at(13)).expect("deleted");
  assert!(db.task(run.task()).expect("read").is_none());
  assert!(db.run(run.id()).expect("read").is_none());
  assert!(db.events(run.id()).expect("read").is_empty());
  assert!(db.task(other_task).expect("read").is_some(), "the other task stays");
  assert!(
    !db.events(other.id()).expect("read").is_empty(),
    "its run keeps its history"
  );

  let connection =
    rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("the database opens");
  let guarded = connection.execute("DELETE FROM run_events WHERE run_id = ?1", [other.id().to_string()]);
  assert!(guarded.is_err(), "run events are append-only again");
}
