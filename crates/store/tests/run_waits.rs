//! Which run waits on which task (ORC-759): a run waits on one task at a time, its waits are read by the project, by
//! the waited task and by the run, and a wait ends once with how it ended.
//!
//! `cargo test -p orchester-store --test run_waits run_waits_`

mod support;

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  run::{BranchName, NewRun, Run, RunWaitsOn, WaitReason, WaitingRun, WaitsOnEnd, WaitsOnError},
  store::{RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
};
use orchester_store::{KeptRunWait, ProjectDb, RunWaitEnd, RunWaitEnded, StoreError};
use support::{at, name, started_run};

fn task(db: &mut ProjectDb, title: &str) -> TaskId {
  let details = TaskDetails {
    title: name(title),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  db.create_tasks(IdempotencyKey::generate(), vec![details], at(8))
    .expect("task")[0]
    .id()
}

/// Another recorded run, of a task of its own, with `run`'s snapshot.
fn other_run(db: &mut ProjectDb, run: &Run) -> Run {
  let task = task(db, "Say goodbye");
  let new = NewRun {
    task,
    snapshot: run.snapshot().clone(),
    branch: BranchName::new(&format!("orchester/{task}")).expect("branch"),
    base: run.base().clone(),
    base_branch: None,
    base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
    budget: None,
    max_duration: None,
  };
  db.create_run(IdempotencyKey::generate(), new, at(9)).expect("run")
}

fn wait(db: &ProjectDb, run: &Run, waited: TaskId) -> RunWaitsOn {
  let project = db.project().id();
  let waiting = WaitingRun {
    project,
    run: run.id(),
    task: run.task(),
    base: BranchName::new("master").expect("branch"),
    waits_for: WaitReason::Message,
  };
  RunWaitsOn::new(waiting, waited, project).expect("a valid wait")
}

fn active(wait: &RunWaitsOn, since: i64) -> KeptRunWait {
  KeptRunWait {
    wait: wait.clone(),
    since: at(since),
    ended: None,
  }
}

#[test]
fn run_waits_are_read_by_project_by_waited_task_and_by_run() {
  let (_repo, mut db, run, _) = started_run();
  let other = other_run(&mut db, &run);
  let (waited, elsewhere) = (task(&mut db, "Build it"), task(&mut db, "Test it"));
  assert_eq!(db.run_wait(run.id()).expect("read"), None, "never waited");

  let first = wait(&db, &run, waited);
  let second = wait(&db, &other, waited);
  let third_run = other_run(&mut db, &run);
  let third = wait(&db, &third_run, elsewhere);
  db.keep_run_wait(&first, at(20)).expect("kept");
  db.keep_run_wait(&second, at(21)).expect("kept");
  db.keep_run_wait(&third, at(22)).expect("kept");

  assert_eq!(
    db.active_run_waits().expect("read"),
    vec![active(&first, 20), active(&second, 21), active(&third, 22)]
  );
  assert_eq!(
    db.run_waits_on_task(waited).expect("read"),
    vec![active(&first, 20), active(&second, 21)]
  );
  assert_eq!(db.run_waits_on_task(run.task()).expect("read"), Vec::new());
  assert_eq!(db.run_wait(other.id()).expect("read"), Some(active(&second, 21)));
}

#[test]
fn run_waits_keep_one_active_wait_per_run() {
  let (_repo, mut db, run, _) = started_run();
  let (waited, another) = (task(&mut db, "Build it"), task(&mut db, "Test it"));
  let first = wait(&db, &run, waited);
  db.keep_run_wait(&first, at(20)).expect("kept");

  let refused = db.keep_run_wait(&wait(&db, &run, another), at(21));
  assert!(
    matches!(refused, Err(StoreError::RunWaitsOn(WaitsOnError::AlreadyWaits(kept))) if kept == waited),
    "{refused:?}"
  );
  assert_eq!(db.active_run_waits().expect("read"), vec![active(&first, 20)]);

  assert!(db.end_run_wait(run.id(), RunWaitEnd::Merged, at(30)).expect("ended"));
  let next = wait(&db, &run, another);
  db.keep_run_wait(&next, at(31)).expect("kept once the first ended");
  assert_eq!(db.run_wait(run.id()).expect("read"), Some(active(&next, 31)));
}

#[test]
fn run_waits_end_once_with_how_they_ended() {
  let (_repo, mut db, run, _) = started_run();
  let other = other_run(&mut db, &run);
  let waited = task(&mut db, "Build it");
  let (first, second) = (wait(&db, &run, waited), wait(&db, &other, waited));
  db.keep_run_wait(&first, at(20)).expect("kept");
  db.keep_run_wait(&second, at(21)).expect("kept");

  let merged = RunWaitEnd::from(
    &first
      .end(&orchester_core::run::WaitsOnChange::Merged {
        task: waited,
        branch: BranchName::new("master").expect("branch"),
      })
      .expect("the merge ends it"),
  );
  assert_eq!(merged, RunWaitEnd::Merged);
  assert!(db.end_run_wait(run.id(), merged, at(30)).expect("ended"));
  assert!(
    db.end_run_wait(other.id(), RunWaitEnd::from(&WaitsOnEnd::Dropped), at(31))
      .expect("ended")
  );
  assert!(
    !db.end_run_wait(run.id(), RunWaitEnd::Canceled, at(32)).expect("read"),
    "an ended wait ends once"
  );

  assert_eq!(db.active_run_waits().expect("read"), Vec::new());
  assert_eq!(db.run_waits_on_task(waited).expect("read"), Vec::new());
  let ended = |wait: &RunWaitsOn, since, end, when| KeptRunWait {
    wait: wait.clone(),
    since: at(since),
    ended: Some(RunWaitEnded { at: at(when), end }),
  };
  assert_eq!(
    db.run_wait(run.id()).expect("read"),
    Some(ended(&first, 20, RunWaitEnd::Merged, 30))
  );
  assert_eq!(
    db.run_wait(other.id()).expect("read"),
    Some(ended(&second, 21, RunWaitEnd::RunEnded, 31))
  );
}

/// ORC-766: a wait also ends as its run was answered first; the table rebuilt for it keeps one active wait per run and
/// one end per wait.
#[test]
fn run_waits_end_as_answered_and_keep_their_rules_on_the_rebuilt_table() {
  let (repo, mut db, run, _) = started_run();
  let (waited, another) = (task(&mut db, "Build it"), task(&mut db, "Test it"));
  let first = wait(&db, &run, waited);
  db.keep_run_wait(&first, at(20)).expect("kept");
  assert!(
    db.keep_run_wait(&wait(&db, &run, another), at(21)).is_err(),
    "one active wait per run"
  );
  assert!(db.end_run_wait(run.id(), RunWaitEnd::Answered, at(30)).expect("ended"));
  assert!(
    !db.end_run_wait(run.id(), RunWaitEnd::Merged, at(31)).expect("read"),
    "it ends once"
  );
  drop(db);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(
    db.run_wait(run.id()).expect("read"),
    Some(KeptRunWait {
      wait: first,
      since: at(20),
      ended: Some(RunWaitEnded {
        at: at(30),
        end: RunWaitEnd::Answered
      }),
    })
  );
}

#[test]
fn run_waits_refuse_a_run_that_is_not_recorded() {
  let (_repo, mut db, run, _) = started_run();
  let waited = task(&mut db, "Build it");
  let project = db.project().id();
  let unknown = WaitingRun {
    project,
    run: orchester_core::id::RunId::generate(),
    task: run.task(),
    base: BranchName::new("master").expect("branch"),
    waits_for: WaitReason::Plan,
  };
  let wait = RunWaitsOn::new(unknown, waited, project).expect("a valid wait");
  let refused = db.keep_run_wait(&wait, at(20));
  assert!(
    matches!(refused, Err(StoreError::RunNotFound(id)) if id == wait.run()),
    "{refused:?}"
  );
  assert_eq!(db.active_run_waits().expect("read"), Vec::new());
}

#[test]
fn run_waits_refuse_a_wait_naming_another_task_as_its_runs() {
  let (_repo, mut db, run, _) = started_run();
  let (stale, waited) = (task(&mut db, "Build it"), task(&mut db, "Test it"));
  let project = db.project().id();
  let wrong = |task, waited| {
    let waiting = WaitingRun {
      project,
      run: run.id(),
      task,
      base: BranchName::new("master").expect("branch"),
      waits_for: WaitReason::Message,
    };
    RunWaitsOn::new(waiting, waited, project).expect("valid on its own")
  };

  // The run's real task as the waited one: read back, it would wait on its own task.
  let own = wrong(stale, run.task());
  let refused = db.keep_run_wait(&own, at(20));
  assert!(
    matches!(refused, Err(StoreError::RunWaitOfAnotherTask { run: id, recorded, named })
      if id == run.id() && recorded == run.task() && named == stale),
    "{refused:?}"
  );
  // Another task: read back, it would be another wait than the one kept.
  let refused = db.keep_run_wait(&wrong(stale, waited), at(21));
  assert!(
    matches!(refused, Err(StoreError::RunWaitOfAnotherTask { .. })),
    "{refused:?}"
  );
  assert_eq!(db.active_run_waits().expect("read"), Vec::new());
  assert_eq!(db.run_wait(run.id()).expect("read"), None);
}

#[test]
fn run_waits_refuse_a_waited_task_that_is_not_recorded() {
  let (_repo, mut db, run, _) = started_run();
  let unknown = TaskId::generate();
  let refused = db.keep_run_wait(&wait(&db, &run, unknown), at(20));
  assert!(
    matches!(refused, Err(StoreError::TaskNotFound(id)) if id == unknown),
    "{refused:?}"
  );
  assert_eq!(db.active_run_waits().expect("read"), Vec::new());
}

#[test]
fn run_waits_take_the_last_kept_as_a_runs_latest_whatever_its_since() {
  let (_repo, mut db, run, _) = started_run();
  let (first, second) = (task(&mut db, "Build it"), task(&mut db, "Test it"));
  let earlier = wait(&db, &run, first);
  db.keep_run_wait(&earlier, at(40)).expect("kept");
  assert!(db.end_run_wait(run.id(), RunWaitEnd::Canceled, at(41)).expect("ended"));
  // The clock went back: the later wait starts at an earlier time.
  let later = wait(&db, &run, second);
  db.keep_run_wait(&later, at(30)).expect("kept");
  assert_eq!(db.run_wait(run.id()).expect("read"), Some(active(&later, 30)));
}
