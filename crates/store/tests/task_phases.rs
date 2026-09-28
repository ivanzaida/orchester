//! The phase of each task from its latest run, as All projects reads it through a read-only
//! connection to count a project's active runs and the runs that need the user.

mod support;

use orchester_core::{id::IdempotencyKey, run::TaskPhase, store::TaskStore, task::TaskDetails};
use orchester_store::ProjectDb;

use self::support::{at, name, started_run};

#[test]
fn task_phases_come_from_the_latest_run_or_the_task() {
  let (repo, mut db, _run, _) = started_run();
  let draft = TaskDetails {
    title: name("Write the release notes"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: None,
  };
  db.create_tasks(IdempotencyKey::generate(), vec![draft], at(10))
    .expect("a draft task");
  drop(db);

  let read = ProjectDb::open_read_only(repo.path()).expect("the project opens to be read");
  assert_eq!(
    read.task_phases().expect("phases"),
    [TaskPhase::Running, TaskPhase::Draft]
  );
}
