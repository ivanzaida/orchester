//! Editing a task in place: each existing run keeps the prompt content it started with.

mod support;

use std::{fs, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, ResourceId, RunId},
  project::{Project, ProjectKey},
  resource::VersionNumber,
  store::TaskStore,
  task::{EditError, TaskDetails, TaskLimits, TaskStatus},
  tokens::TokenBudget,
};
use orchester_store::{ProjectDb, StoreError};

use self::support::{at, name, started_run};

fn draft(title: &str) -> TaskDetails {
  TaskDetails {
    title: name(title),
    brief: String::new(),
    acceptance_criteria: vec!["Lists repositories.".into()],
    pipeline: None,
    limits: None,
  }
}

fn edited() -> TaskDetails {
  let limits =
    TaskLimits::new(TokenBudget::new(900_000).expect("above zero"), Duration::from_secs(600)).expect("positive limits");
  TaskDetails {
    brief: "Let users switch repositories.".into(),
    acceptance_criteria: vec!["Remembers the choice.".into(), "Shows the branch.".into()],
    pipeline: Some(ResourceId::generate()),
    limits: Some(limits),
    ..draft("Repository switcher")
  }
}

#[test]
fn task_edits_update_a_draft_in_place() {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");
  let created = db
    .create_tasks(IdempotencyKey::generate(), vec![draft("One"), draft("Two")], at(1))
    .expect("drafts");

  let details = edited();
  let updated = db
    .update_task(created[0].id(), created[0].revision(), details.clone())
    .expect("the draft is edited")
    .saved()
    .expect("nothing changed it meanwhile");
  assert_eq!(updated.details(), &details);
  assert_eq!((updated.id(), updated.key()), (created[0].id(), created[0].key()));
  assert_eq!(updated.status(), TaskStatus::Draft);
  drop(db);

  let db = ProjectDb::open(repo.path()).expect("reopen");
  let stored = db.tasks().expect("tasks");
  assert_eq!(stored.len(), 2, "no task was added or removed");
  assert_eq!(stored[0], updated);
  assert_eq!(stored[1], created[1]);
}

#[test]
fn task_with_a_run_can_be_edited_without_changing_its_run_snapshot() {
  let (repo, mut db, run, _) = started_run();
  let before = db.task(run.task()).expect("read").expect("the task exists");
  let changed = edited();
  let updated = db
    .update_task(run.task(), before.revision(), changed.clone())
    .expect("edit started task")
    .saved()
    .expect("nothing changed it meanwhile");
  assert_eq!(updated.details(), &changed);
  assert_eq!(
    (updated.id(), updated.key(), updated.status()),
    (before.id(), before.key(), before.status())
  );
  assert_eq!(
    db.task_for_run(run.task(), run.id())
      .expect("run task")
      .map(|task| task.details().clone()),
    Some(before.details().clone())
  );
  db.update_task(
    run.task(),
    updated.revision(),
    TaskDetails {
      title: name("Again"),
      ..changed
    },
  )
  .expect("edit the task again")
  .saved()
  .expect("nothing changed it meanwhile");
  assert_eq!(
    db.task_for_run(run.task(), run.id())
      .expect("run task")
      .map(|task| task.details().clone()),
    Some(before.details().clone())
  );
  drop(db);

  let db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(
    db.task(run.task())
      .expect("task")
      .expect("task exists")
      .details()
      .title
      .as_str(),
    "Again"
  );
  assert_eq!(
    db.task_for_run(run.task(), run.id())
      .expect("run task")
      .map(|task| task.details().clone()),
    Some(before.details().clone())
  );
}

#[test]
fn legacy_run_gets_its_task_snapshot_before_the_first_edit() {
  let (repo, db, run, _) = started_run();
  let before = db.task(run.task()).expect("task").expect("task exists");
  drop(db);
  // Simulate a database migrated from the version before run task snapshots existed.
  let connection = rusqlite::Connection::open(repo.path().join(".orchester/orchester.db")).expect("database");
  connection
    .execute_batch("DROP TRIGGER run_task_snapshots_no_delete; DELETE FROM run_task_snapshots;")
    .expect("remove the new snapshot");
  drop(connection);

  let mut db = ProjectDb::open(repo.path()).expect("reopen");
  db.update_task(run.task(), before.revision(), edited())
    .expect("edit migrated task")
    .saved()
    .expect("nothing changed it meanwhile");
  assert_eq!(
    db.task_for_run(run.task(), run.id())
      .expect("run task")
      .map(|task| task.details().clone()),
    Some(before.details().clone())
  );
}

#[test]
fn task_for_run_rejects_a_run_of_another_task_or_an_unknown_run() {
  let (_repo, mut db, run, _) = started_run();
  let other = db
    .create_tasks(IdempotencyKey::generate(), vec![draft("Other")], at(2))
    .expect("another task")
    .remove(0);
  assert_eq!(db.task_for_run(other.id(), run.id()).expect("lookup"), None);
  assert_eq!(db.task_for_run(run.task(), RunId::generate()).expect("lookup"), None);
}

#[test]
fn task_edits_are_refused_for_a_queued_task() {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![edited()], at(1))
    .expect("draft")
    .remove(0);
  task.queue(VersionNumber::FIRST).expect("a complete draft queues");
  let task = db.save_task(&task).expect("queued").saved().expect("queued as read");
  assert!(matches!(
    db.update_task(task.id(), task.revision(), draft("Other")),
    Err(StoreError::TaskNotEditable {
      source: EditError::QueuedWithoutRun,
      ..
    })
  ));
  assert_eq!(db.task(task.id()).expect("read"), Some(task));
}
