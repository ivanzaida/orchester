//! The sources listed on the project and on each task (M12): each list replaced as a whole, each path once, in order;
//! a run's union is stored with its snapshot and read back as it was when the run started.

mod support;

use orchester_core::{id::TaskId, source::SourcePath, store::RunStore};
use orchester_store::{ProjectDb, StoreError};
use support::started_run;

fn source(name: &str) -> SourcePath {
  SourcePath::new(&std::env::temp_dir().join(name).to_string_lossy()).expect("an absolute path")
}

#[test]
fn sources_of_the_project_and_its_tasks_are_kept_in_order() {
  let (repo, mut db, run, _) = started_run();
  assert!(db.project_sources().expect("read").is_empty());
  db.set_project_sources(&[source("deps"), source("docs"), source("deps")])
    .expect("set");
  assert_eq!(db.project_sources().expect("read"), [source("deps"), source("docs")]);
  db.set_project_sources(&[source("docs")]).expect("replaced");
  assert_eq!(db.project_sources().expect("read"), [source("docs")]);
  let task = run.task();
  assert!(db.task_sources(task).expect("read").is_empty());
  db.set_task_sources(task, &[source("vendor"), source("deps")])
    .expect("set");
  assert_eq!(db.task_sources(task).expect("read"), [source("vendor"), source("deps")]);
  let other = TaskId::generate();
  assert!(matches!(
    db.set_task_sources(other, &[source("x")]),
    Err(StoreError::TaskNotFound(task)) if task == other
  ));
  drop(db);
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.project_sources().expect("read"), [source("docs")]);
  assert_eq!(
    reopened.task_sources(task).expect("read"),
    [source("vendor"), source("deps")]
  );
}

/// A run's sources travel with its snapshot: what it started with, whatever the lists say later.
#[test]
fn sources_of_a_run_stay_as_it_started() {
  let (_repo, mut db, run, _) = started_run();
  assert!(run.snapshot().sources().is_empty());
  db.set_project_sources(&[source("later")]).expect("set");
  let stored = db.run(run.id()).expect("read").expect("the run");
  assert!(stored.snapshot().sources().is_empty());
}
