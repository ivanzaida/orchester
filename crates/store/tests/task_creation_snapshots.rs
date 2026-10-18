//! What a task asked for when it was created is kept as it was: every edit leaves it, and a run's start
//! version, as they were; a task from before creation versions were kept, or a run from before run versions,
//! reads as having none rather than as its current content; a shared task keeps one only when this computer
//! created it and the server confirmed it unchanged.
//!
//! `cargo test -p orchester-store --test task_creation_snapshots <filter>`

mod support;

use std::{num::NonZeroU32, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, ServerId, TaskId},
  project::ProjectKey,
  resource::ResourceKind,
  sharing::{MemberRole, TaskRevision},
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits, TaskStatus, organization::TaskOrganization},
  tokens::TokenBudget,
};
use orchester_store::{CachedTask, ProjectDb, SharingLink};
use support::{at, name, started_run};

fn requirements(title: &str) -> TaskDetails {
  TaskDetails {
    title: name(title),
    brief: "Retry failed deliveries with backoff.".into(),
    acceptance_criteria: vec!["Retries back off.".into(), "Every attempt is recorded.".into()],
    pipeline: None,
    limits: TaskLimits::new(
      Some(TokenBudget::new(500_000).expect("above zero")),
      Some(Duration::from_secs(600)),
    )
    .expect("limits"),
  }
}

fn edited(details: &TaskDetails) -> TaskDetails {
  TaskDetails {
    title: name("Retry webhooks, edited"),
    brief: "Retry with jitter too.".into(),
    acceptance_criteria: vec!["Retries back off.".into(), "Cancellation stops retries.".into()],
    limits: TaskLimits::NONE,
    ..details.clone()
  }
}

fn database(path: &std::path::Path) -> rusqlite::Connection {
  rusqlite::Connection::open(path.join(".orchester/orchester.db")).expect("database")
}

#[test]
fn creation_snapshots_keep_what_a_new_task_asked_for_through_edits() {
  let (repo, mut db, _run, _) = started_run();
  let created = db
    .create_tasks(IdempotencyKey::generate(), vec![requirements("Retry webhooks")], at(10))
    .expect("created")
    .remove(0);
  assert_eq!(
    db.task_creation_details(created.id()).expect("read"),
    Some(requirements("Retry webhooks"))
  );
  let changed = edited(created.details());
  let saved = db
    .update_task(created.id(), created.revision(), changed.clone())
    .expect("edit")
    .saved()
    .expect("saved as read");
  assert_eq!(saved.details(), &changed, "the task itself changed");
  drop(db);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(
    db.task_creation_details(created.id()).expect("read"),
    Some(requirements("Retry webhooks")),
    "the creation version is as it was"
  );
}

#[test]
fn creation_snapshots_an_organized_draft_keeps_its_first_save() {
  let (_repo, mut db, _run, _) = started_run();
  let batch = IdempotencyKey::generate();
  let task = db
    .create_organized_task(batch, requirements("Organized"), &TaskOrganization::default(), at(10))
    .expect("created");
  // The same form sent again edits its draft; the creation version stays the first one.
  db.create_organized_task(batch, edited(task.details()), &TaskOrganization::default(), at(11))
    .expect("sent again");
  assert_eq!(
    db.task_creation_details(task.id()).expect("read"),
    Some(requirements("Organized"))
  );
}

#[test]
fn creation_snapshots_cannot_be_changed_or_deleted() {
  let (repo, mut db, _run, _) = started_run();
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![requirements("Fixed")], at(10))
    .expect("created")
    .remove(0);
  drop(db);
  let connection = database(repo.path());
  let id = task.id().to_string();
  assert!(
    connection
      .execute(
        "UPDATE task_creation_snapshots SET title = 'Other' WHERE task_id = ?1",
        [&id]
      )
      .is_err()
  );
  assert!(
    connection
      .execute("DELETE FROM task_creation_snapshots WHERE task_id = ?1", [&id])
      .is_err()
  );
}

#[test]
fn creation_snapshots_an_older_task_has_none_and_its_run_keeps_its_start() {
  let (repo, db, run, _) = started_run();
  let before = db.task(run.task()).expect("read").expect("task");
  let started = db.run_start_details(run.id()).expect("read").expect("kept at start");
  assert_eq!(&started, before.details());
  drop(db);
  // A task created before creation versions were kept.
  let connection = database(repo.path());
  connection
    .execute_batch("DROP TRIGGER task_creation_snapshots_no_delete; DELETE FROM task_creation_snapshots;")
    .expect("as before creation versions");
  drop(connection);
  let mut db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(db.task_creation_details(run.task()).expect("read"), None);
  // A task with a run stays complete.
  let delivery = db.resources(ResourceKind::Pipeline).expect("pipelines").remove(0).id();
  let changed = TaskDetails {
    pipeline: Some(delivery),
    ..edited(before.details())
  };
  db.update_task(run.task(), before.revision(), changed)
    .expect("edit")
    .saved()
    .expect("saved as read");
  assert_eq!(
    db.task_creation_details(run.task()).expect("read"),
    None,
    "never made up"
  );
  assert_eq!(
    db.run_start_details(run.id()).expect("read"),
    Some(started),
    "the run keeps what it started with"
  );
}

#[test]
fn creation_snapshots_a_run_from_before_run_versions_has_none() {
  let (repo, db, run, _) = started_run();
  drop(db);
  let connection = database(repo.path());
  connection
    .execute_batch("DROP TRIGGER run_task_snapshots_no_delete; DELETE FROM run_task_snapshots;")
    .expect("as before run versions");
  drop(connection);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(
    db.run_start_details(run.id()).expect("read"),
    None,
    "not the task's current content"
  );
}

fn shared(id: TaskId, number: u32, revision: TaskRevision) -> CachedTask {
  let details = requirements("Shared retries");
  CachedTask {
    id,
    number: NonZeroU32::new(number).expect("number"),
    revision,
    title: details.title,
    brief: details.brief,
    criteria: details.acceptance_criteria,
    pipeline: Some("delivery".into()),
    limits: details.limits,
    status: TaskStatus::Draft,
    organization: TaskOrganization::default(),
    former_key: None,
    created_by: "Alice Moreau".into(),
    created_at: at(20),
    updated_by: "Alice Moreau".into(),
    updated_at: at(20),
    claim: None,
    run: None,
  }
}

#[test]
fn creation_snapshots_a_shared_task_keeps_one_only_when_created_here_unchanged() {
  let (_repo, mut db, _run, _) = started_run();
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("ORC").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: at(15),
    cursor: 1,
    last_synced_at: at(15),
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (&[], &Default::default()))
    .expect("shared");
  let delivery = db.resources(ResourceKind::Pipeline).expect("pipelines").remove(0).id();
  let here = shared(TaskId::generate(), 5, TaskRevision::FIRST);
  db.cache_shared_tasks(std::slice::from_ref(&here), Some("batch-here"))
    .expect("cached");
  let kept = db.task_creation_details(here.id).expect("read").expect("created here");
  assert_eq!(kept.title.as_str(), "Shared retries");
  assert_eq!(kept.pipeline, Some(delivery), "this computer's pipeline of its slug");
  let elsewhere = shared(TaskId::generate(), 6, TaskRevision::FIRST);
  db.cache_shared_tasks(std::slice::from_ref(&elsewhere), None)
    .expect("cached");
  assert_eq!(db.task_creation_details(elsewhere.id).expect("read"), None);
  let changed = shared(TaskId::generate(), 7, TaskRevision::FIRST.next().expect("next"));
  db.cache_shared_tasks(std::slice::from_ref(&changed), Some("batch-replayed"))
    .expect("cached");
  assert_eq!(
    db.task_creation_details(changed.id).expect("read"),
    None,
    "changed before this computer read it"
  );
}
