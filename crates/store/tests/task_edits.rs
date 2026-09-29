//! Editing a saved draft in place: the same task, with its number, takes the new details; a task
//! with a run or out of the drafts is refused and left as it was.

mod support;

use std::{fs, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, ResourceId},
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
    .update_draft(created[0].id(), details.clone())
    .expect("the draft is edited");
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
fn task_edits_are_refused_for_a_task_with_a_run() {
  let (_repo, mut db, run, _) = started_run();
  let before = db.task(run.task()).expect("read").expect("the task exists");
  assert!(matches!(
    db.update_draft(run.task(), edited()),
    Err(StoreError::TaskNotEditable {
      source: EditError::HasRun,
      ..
    })
  ));
  assert_eq!(db.task(run.task()).expect("read"), Some(before));
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
  db.save_task(&task).expect("queued");
  assert!(matches!(
    db.update_draft(task.id(), draft("Other")),
    Err(StoreError::TaskNotEditable {
      source: EditError::NotDraft(TaskStatus::Queued),
      ..
    })
  ));
  assert_eq!(db.task(task.id()).expect("read"), Some(task));
}
