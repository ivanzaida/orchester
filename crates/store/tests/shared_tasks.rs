//! Task revisions and the cache of a shared project (`docs/architecture/server.md`, "The desktop's
//! trait boundary", "Desktop data", "Migration"): every stored change raises a task's revision and a
//! change made on an older one is a conflict; a shared project's tasks are written only from what
//! the server confirmed, with the pipeline by slug, and refuse local writes; a run of a shared task
//! records its workflow status for the holder's desktop to send; Make local keeps every cached task;
//! and a populated database from before sharing opens with every task at its first revision.
//!
//! `cargo test -p orchester-store --test shared_tasks <filter>`

mod support;

use std::num::NonZeroU32;

use orchester_core::{
  id::{ClaimId, IdempotencyKey, ProjectId, ServerId, TagId, UserId},
  project::ProjectKey,
  resource::{ResourceKind, VersionNumber},
  run::TaskPhase,
  sharing::{ClaimKind, MemberRole, TaskRevision},
  store::{ResourceStore, RunStore, TaskStore},
  task::{
    TaskDetails, TaskStatus, TaskUpdate,
    organization::{ChangeActor, Priority, Tag, TaskOrganization, WorkflowStatus},
  },
};
use orchester_store::{CachedClaim, CachedRunStatus, CachedTask, ProjectDb, SharedPerson, SharingLink, StoreError};
use support::{at, copied_at_version, name, started_run};

/// The schema before shared projects: ORC-63's worktree removals (40). Migrations are append-only.
const BEFORE_SHARING: u32 = 40;

fn details(title: &str) -> TaskDetails {
  TaskDetails {
    title: name(title),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: None,
  }
}

/// The fixture's task made complete, on the pipeline `delivery`, as a task with a run must stay.
fn completed(db: &mut ProjectDb, task: orchester_core::id::TaskId) -> orchester_core::task::Task {
  let read = db.task(task).expect("read").expect("task");
  let delivery = db.resources(ResourceKind::Pipeline).expect("pipelines").remove(0).id();
  let details = TaskDetails {
    brief: "Say hello in the README.".into(),
    acceptance_criteria: vec!["The README says hello.".into()],
    pipeline: Some(delivery),
    ..read.details().clone()
  };
  db.update_task(task, read.revision(), details)
    .expect("complete the task")
    .saved()
    .expect("saved as read")
}

fn revision(value: u64) -> TaskRevision {
  TaskRevision::try_from(value).expect("a revision")
}

fn person(name: &str) -> SharedPerson {
  SharedPerson {
    id: UserId::generate(),
    name: name.to_owned(),
  }
}

fn link(key: &str) -> SharingLink {
  SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new(key).expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: at(100),
    cursor: 7,
    last_synced_at: at(100),
    removed_at: None,
  }
}

/// `task` of the project as the server confirmed it after `by` changed it.
fn cached(db: &ProjectDb, task: &orchester_core::task::Task, pipeline: Option<&str>, by: &str) -> CachedTask {
  let details = task.details();
  CachedTask {
    id: task.id(),
    number: task.key().number(),
    revision: task.revision(),
    title: details.title.clone(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: pipeline.map(str::to_owned),
    limits: details.limits,
    status: task.status(),
    organization: db.task_organization(task.id()).expect("organization"),
    former_key: None,
    created_by: by.to_owned(),
    created_at: task.created_at(),
    updated_by: by.to_owned(),
    updated_at: at(100),
    claim: None,
    run: None,
  }
}

#[test]
fn shared_tasks_a_change_on_an_older_revision_is_a_conflict_and_changes_nothing() {
  let (_repo, mut db, run, _) = started_run();
  let read = completed(&mut db, run.task());
  let mut changed = read.details().clone();
  changed.brief = "Say hello at the end of the README.".into();
  let saved = db
    .update_task(read.id(), read.revision(), changed)
    .expect("edit")
    .saved()
    .expect("saved on the revision read");
  assert_eq!(saved.revision(), read.revision().next().expect("raised"));

  let mut other = read.details().clone();
  other.brief = "Say goodbye.".into();
  let TaskUpdate::Conflict { current } = db.update_task(read.id(), read.revision(), other).expect("edit") else {
    panic!("the task changed after it was read");
  };
  assert_eq!(current, saved);
  assert!(matches!(
    db.save_task(&read).expect("save"),
    TaskUpdate::Conflict { current } if current == saved
  ));
  assert_eq!(db.task(read.id()).expect("read"), Some(saved));
}

#[test]
fn shared_tasks_an_organization_change_raises_the_revision() {
  let (_repo, mut db, run, _) = started_run();
  let before = db.task(run.task()).expect("read").expect("task");
  let value = TaskOrganization::default().with_priority(Priority::High);
  db.set_task_organization(before.id(), &value, ChangeActor::Person, at(20))
    .expect("organize");
  let after = db.task(before.id()).expect("read").expect("task");
  assert_eq!(after.revision(), before.revision().next().expect("raised"));
}

#[test]
fn shared_tasks_the_cache_keeps_the_server_s_tasks_and_refuses_local_writes() {
  let (_repo, mut db, run, _) = started_run();
  let task = db.task(run.task()).expect("read").expect("task");
  let draft = db
    .create_tasks(IdempotencyKey::generate(), vec![details("Fix reconnect")], at(30))
    .expect("draft")
    .remove(0);
  let review = Tag::new(TagId::generate(), name("Review"));
  let mut cached_draft = cached(&db, &draft, Some("design-review"), "Alice Moreau");
  cached_draft.revision = revision(4);
  cached_draft.organization = TaskOrganization::default().with_tags(vec![review.id()]);
  let alice = person("Alice Moreau");
  cached_draft.claim = Some(CachedClaim {
    id: ClaimId::generate(),
    holder: alice.clone(),
    kind: ClaimKind::Manual,
    since: at(90),
    heartbeat_at: at(95),
    stale: false,
    taken_over_from: None,
  });
  let link = link("ORC");
  db.start_sharing(
    &link,
    std::slice::from_ref(&review),
    &[cached(&db, &task, Some("delivery"), "Alice Moreau"), cached_draft],
  )
  .expect("share");

  assert_eq!(db.sharing().expect("link"), Some(link));
  assert_eq!(db.project_tags().expect("tags"), vec![review.clone()]);
  // This computer has the pipeline `delivery`, not `design-review`.
  let delivery = db.resources(ResourceKind::Pipeline).expect("pipelines").remove(0).id();
  let shared = db.task(task.id()).expect("read").expect("task");
  assert_eq!(shared.details().pipeline, Some(delivery));
  let other = db.task(draft.id()).expect("read").expect("task");
  assert_eq!(other.details().pipeline, None);
  assert_eq!(other.revision(), revision(4));
  assert_eq!(other.organization().tags(), [review.id()]);
  let info = db.shared_task_info(draft.id()).expect("info").expect("shared");
  assert_eq!(info.pipeline_slug.as_deref(), Some("design-review"));
  assert_eq!(info.claim.map(|claim| claim.holder), Some(alice.clone()));
  assert_eq!(db.claims_held_by(alice.id).expect("claims"), vec![draft.id()]);

  assert!(matches!(
    db.update_task(draft.id(), other.revision(), details("Changed here")),
    Err(StoreError::SharedTask(id)) if id == draft.id()
  ));
  assert!(matches!(
    db.set_task_organization(draft.id(), &TaskOrganization::default(), ChangeActor::Person, at(40)),
    Err(StoreError::SharedTask(_))
  ));
  assert!(matches!(
    db.create_tasks(IdempotencyKey::generate(), vec![details("New here")], at(40)),
    Err(StoreError::SharedProject)
  ));
  assert!(matches!(
    db.create_project_tag(name("Local")),
    Err(StoreError::SharedProject)
  ));
  assert_eq!(db.task(draft.id()).expect("read"), Some(other));
}

#[test]
fn shared_tasks_a_change_from_the_server_keeps_the_runs_content_and_the_local_pin() {
  let (_repo, mut db, run, _) = started_run();
  let mut task = completed(&mut db, run.task());
  task.queue(VersionNumber::FIRST).expect("a complete task queues");
  let task = db.save_task(&task).expect("queue").saved().expect("saved as read");
  let pin = task.pipeline_version();
  db.start_sharing(
    &link("ORC"),
    &[],
    &[cached(&db, &task, Some("delivery"), "Alice Moreau")],
  )
  .expect("share");

  let mut edited = cached(&db, &task, Some("delivery"), "Oleh Petrenko");
  edited.title = name("Say hello twice");
  edited.revision = task.revision().next().expect("next");
  edited.run = Some(CachedRunStatus {
    holder: person("Oleh Petrenko"),
    number: NonZeroU32::new(3).expect("number"),
    phase: TaskPhase::Running,
    step: Some("Implement".into()),
    started_at: at(110),
    phase_since: at(111),
    updated_at: at(112),
  });
  db.cache_shared_tasks(&[edited], None).expect("cache");
  let now = db.task(task.id()).expect("read").expect("task");
  assert_eq!(now.details().title.as_str(), "Say hello twice");
  assert_eq!(
    now.pipeline_version(),
    pin,
    "the local pin stays while the task is queued"
  );
  assert_eq!(
    db.task_for_run(task.id(), run.id())
      .expect("run task")
      .map(|run| run.details().title.clone()),
    Some(task.details().title.clone()),
    "the run keeps the content it started with"
  );
  let info = db.shared_task_info(task.id()).expect("info").expect("shared");
  assert_eq!(info.run.map(|run| run.phase), Some(TaskPhase::Running));
  assert_eq!(info.updated_by.as_deref(), Some("Oleh Petrenko"));
}

#[test]
fn shared_tasks_a_run_of_a_shared_task_records_its_status_for_the_server() {
  let (_repo, mut db, mut run, execution) = started_run();
  let task = db.task(run.task()).expect("read").expect("task");
  let organization = db.task_organization(task.id()).expect("organization");
  db.start_sharing(
    &link("ORC"),
    &[],
    &[cached(&db, &task, Some("delivery"), "Alice Moreau")],
  )
  .expect("share");
  let events = run.start_step(execution, at(120)).expect("start step");
  db.append_events(&run, &events).expect("stored");
  let events = run.complete_step(execution, at(121)).expect("complete step");
  db.append_events(&run, &events).expect("stored");
  assert_eq!(
    db.task_organization(task.id()).expect("organization"),
    organization,
    "the cache changes only with what the server confirmed"
  );
  let pending = db.automatic_statuses().expect("pending");
  assert_eq!(pending, vec![(task.id(), WorkflowStatus::InReview, at(121))]);
  db.clear_automatic_status(task.id(), at(120)).expect("an older one");
  assert_eq!(db.automatic_statuses().expect("pending").len(), 1);
  db.clear_automatic_status(task.id(), at(121)).expect("sent");
  assert!(db.automatic_statuses().expect("pending").is_empty());
}

#[test]
fn shared_tasks_make_local_keeps_every_cached_task() {
  let (_repo, mut db, run, _) = started_run();
  let task = db.task(run.task()).expect("read").expect("task");
  let mut theirs = cached(&db, &task, Some("delivery"), "Alice Moreau");
  theirs.id = orchester_core::id::TaskId::generate();
  theirs.number = NonZeroU32::new(41).expect("number");
  theirs.status = TaskStatus::Draft;
  theirs.title = name("Alice's task");
  theirs.claim = Some(CachedClaim {
    id: ClaimId::generate(),
    holder: person("Alice Moreau"),
    kind: ClaimKind::Run,
    since: at(90),
    heartbeat_at: at(95),
    stale: false,
    taken_over_from: None,
  });
  let tag = Tag::new(TagId::generate(), name("Desktop"));
  db.start_sharing(
    &link("ORC"),
    std::slice::from_ref(&tag),
    &[cached(&db, &task, Some("delivery"), "Alice Moreau"), theirs.clone()],
  )
  .expect("share");
  db.record_sharing_state(Some(MemberRole::Member), Some(at(150)), None)
    .expect("state");
  assert_eq!(
    db.sharing().expect("link").map(|link| link.last_synced_at),
    Some(at(150))
  );

  db.make_local().expect("make local");
  assert_eq!(db.sharing().expect("link"), None);
  let tasks = db.tasks().expect("tasks");
  assert_eq!(tasks.len(), 2);
  assert_eq!(tasks[1].key().to_string(), "ORC-41");
  let delivery = db.resources(ResourceKind::Pipeline).expect("pipelines").remove(0).id();
  assert_eq!(tasks[1].details().pipeline, Some(delivery));
  assert_eq!(db.shared_task_info(theirs.id).expect("info"), None);
  assert!(
    db.claims_held_by(theirs.claim.expect("claim").holder.id)
      .expect("claims")
      .is_empty()
  );
  assert_eq!(db.project_tags().expect("tags"), vec![tag]);
  let next = db
    .create_tasks(IdempotencyKey::generate(), vec![details("After")], at(200))
    .expect("a local task again")
    .remove(0);
  assert_eq!(next.key().to_string(), "ORC-42");
  db.update_task(next.id(), next.revision(), details("Edited here"))
    .expect("edit")
    .saved()
    .expect("a local task changes here");
}

/// A populated database of schema 40 opens with every task at its first revision and local, its
/// runs and organization unchanged, and its tasks change as before.
#[test]
fn shared_tasks_migration_keeps_a_populated_database_of_schema_40() {
  let (repo, mut db, run, _) = started_run();
  let value = TaskOrganization::default().with_priority(Priority::Urgent);
  db.set_task_organization(run.task(), &value, ChangeActor::Person, at(20))
    .expect("organize");
  let before = db.task(run.task()).expect("read").expect("task");
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_SHARING);
  let mut db = ProjectDb::open(old.path()).expect("migrate existing project");
  let task = db.task(run.task()).expect("read").expect("task");
  assert_eq!(task.revision(), TaskRevision::FIRST);
  assert_eq!(task.details(), before.details());
  assert_eq!(task.organization(), &value);
  assert_eq!(task.pipeline_version(), before.pipeline_version());
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
  assert_eq!(db.sharing().expect("link"), None);
  assert_eq!(db.shared_task_info(task.id()).expect("info"), None);
  let saved = completed(&mut db, task.id());
  assert_eq!(saved.revision(), revision(2));
}
