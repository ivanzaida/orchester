use std::{fs, path::Path, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  store::TaskStore,
  task::{TaskDetails, TaskLimits, TaskStatus},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

fn git_repo() -> TempDir {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  repo
}

fn kontur() -> Project {
  Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  )
}

fn details(title: &str) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("valid title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: None,
  }
}

fn complete(title: &str) -> TaskDetails {
  let budget = orchester_core::tokens::TokenBudget::new(1_400_000).expect("above zero");
  let limits = TaskLimits::new(budget, Duration::from_secs(1800)).expect("positive limits");
  TaskDetails {
    brief: "Let users switch repositories.".into(),
    acceptance_criteria: vec!["Lists repositories.".into(), "Remembers the choice.".into()],
    pipeline: Some(ResourceId::generate()),
    limits: Some(limits),
    ..details(title)
  }
}

fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

fn exclude_file(repo: &Path) -> String {
  fs::read_to_string(repo.join(".git/info/exclude")).expect("exclude file")
}

#[test]
fn create_initializes_and_excludes_the_directory_once() {
  let repo = git_repo();
  let project = kontur();
  let db = ProjectDb::create(repo.path(), project.clone()).expect("create");
  assert_eq!(db.project(), &project);
  assert!(repo.path().join(".orchester/orchester.db").is_file());
  assert_eq!(exclude_file(repo.path()).matches("/.orchester/").count(), 1);
  drop(db);

  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.project(), &project);
  assert!(matches!(
    ProjectDb::create(repo.path(), kontur()),
    Err(StoreError::AlreadyInitialized(_))
  ));
}

#[test]
fn create_requires_a_git_repository_and_open_an_initialized_one() {
  let plain = tempfile::tempdir().expect("temporary directory");
  assert!(matches!(
    ProjectDb::create(plain.path(), kontur()),
    Err(StoreError::NotAGitRepository(_))
  ));
  assert!(matches!(
    ProjectDb::open(plain.path()),
    Err(StoreError::NotInitialized(_))
  ));
}

#[test]
fn task_batch_gets_consecutive_keys_and_survives_reopening() {
  let repo = git_repo();
  let mut db = ProjectDb::create(repo.path(), kontur()).expect("create");
  let first = db
    .create_tasks(IdempotencyKey::generate(), vec![details("One"), complete("Two")], now())
    .expect("first batch");
  let second = db
    .create_tasks(IdempotencyKey::generate(), vec![details("Three")], now())
    .expect("second batch");
  let keys: Vec<String> = first.iter().chain(&second).map(|task| task.key().to_string()).collect();
  assert_eq!(keys, ["ORC-1", "ORC-2", "ORC-3"]);
  drop(db);

  let db = ProjectDb::open(repo.path()).expect("reopen");
  let stored = db.tasks().expect("list");
  assert_eq!(stored, [first, second].concat());
}

#[test]
fn repeating_a_batch_returns_the_original_tasks() {
  let repo = git_repo();
  let mut db = ProjectDb::create(repo.path(), kontur()).expect("create");
  let batch = IdempotencyKey::generate();
  let created = db
    .create_tasks(batch, vec![details("One"), details("Two")], now())
    .expect("create");
  let retried = db
    .create_tasks(batch, vec![details("One"), details("Two")], now())
    .expect("retry");
  assert_eq!(retried, created);
  assert_eq!(db.tasks().expect("list").len(), 2);
  assert!(matches!(
    db.create_tasks(IdempotencyKey::generate(), Vec::new(), now()),
    Err(StoreError::EmptyBatch)
  ));
}

#[test]
fn saving_a_queued_task_persists_details_and_status() {
  let repo = git_repo();
  let mut db = ProjectDb::create(repo.path(), kontur()).expect("create");
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![complete("Switcher")], now())
    .expect("create")
    .remove(0);
  task
    .queue(orchester_core::resource::VersionNumber::FIRST)
    .expect("complete task queues");
  db.save_task(&task).expect("save");
  let stored = db.task(task.id()).expect("read").expect("exists");
  assert_eq!(stored, task);
  assert_eq!(stored.status(), TaskStatus::Queued);
  assert_eq!(
    stored.pipeline_version(),
    Some(orchester_core::resource::VersionNumber::FIRST)
  );
}

#[test]
fn old_queued_task_without_a_version_remains_unpinned_after_reopen() {
  let repo = git_repo();
  let mut db = ProjectDb::create(repo.path(), kontur()).expect("create");
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![complete("Legacy")], now())
    .expect("create")
    .remove(0);
  task
    .queue(orchester_core::resource::VersionNumber::FIRST)
    .expect("queue");
  db.save_task(&task).expect("save");
  drop(db);
  let database = repo.path().join(".orchester/orchester.db");
  rusqlite::Connection::open(database)
    .expect("open raw fixture")
    .execute(
      "UPDATE tasks SET pipeline_version = NULL WHERE id = ?1",
      [task.id().to_string()],
    )
    .expect("simulate a pre-migration queued row");
  let loaded = ProjectDb::open(repo.path())
    .expect("reopen")
    .task(task.id())
    .expect("read")
    .expect("task exists");
  assert_eq!(loaded.status(), TaskStatus::Queued);
  assert_eq!(loaded.pipeline_version(), None);
}
