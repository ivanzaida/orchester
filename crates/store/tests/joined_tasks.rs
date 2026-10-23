//! Joining a shared repository (`docs/architecture/server.md`, "Two people share one repository"):
//! the ticked local tasks become the server's with new numbers and their former keys, the unticked
//! ones stay *Not shared* (read-only, out of the list, with their key and tag names), the project
//! takes the server's key and the mapping is kept; *Share task* makes a kept task the server's;
//! *Make local* returns the kept tasks with numbers after the highest and their tags by name; a
//! populated database of the schema before joining opens unchanged.
//!
//! `cargo test -p orchester-store --test joined_tasks joined_`

mod support;

use std::{collections::HashMap, num::NonZeroU32};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, ServerId, TagId, TaskId},
  project::{Project, ProjectKey},
  sharing::{MemberRole, TaskRevision},
  store::TaskStore,
  task::{
    Task, TaskDetails, TaskLimits, TaskStatus,
    organization::{ChangeActor, Priority, Tag, TaskOrganization},
  },
};
use orchester_store::{CachedTask, ProjectDb, SharingLink, StoreError};
use support::{at, copied_at_version, name};
use tempfile::TempDir;

/// The schema before joining: shared projects (41). Migrations are append-only.
const BEFORE_JOINING: u32 = 41;

fn details(title: &str) -> TaskDetails {
  TaskDetails {
    title: name(title),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  }
}

/// Bohdan's project Kontur, key KON, with three tasks; the first two carry the tags CI and Desktop.
fn kontur() -> (TempDir, ProjectDb, Vec<Task>, Vec<Tag>) {
  let repo = tempfile::tempdir().expect("temporary directory");
  std::fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");
  let tasks = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![
        details("Add switcher shortcuts"),
        details("Fix reconnect"),
        details("Retry the flaky test"),
      ],
      at(10),
    )
    .expect("tasks");
  let tags = vec![
    db.create_project_tag(name("CI")).expect("tag"),
    db.create_project_tag(name("Desktop")).expect("tag"),
  ];
  for task in &tasks[..2] {
    let value = TaskOrganization::default()
      .with_priority(Priority::Low)
      .with_tags(tags.iter().map(Tag::id).collect());
    db.set_task_organization(task.id(), &value, ChangeActor::Person, at(20))
      .expect("organize");
  }
  (repo, db, tasks, tags)
}

fn link() -> SharingLink {
  SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("ORC").expect("key"),
    role: Some(MemberRole::Member),
    joined_at: at(100),
    cursor: 7,
    last_synced_at: at(100),
    removed_at: None,
    address: Some("https://orchester.kontur.lan/kontur".into()),
  }
}

/// A task of the server project with `number`.
fn server_task(id: TaskId, number: u32, title: &str, former: Option<&str>) -> CachedTask {
  CachedTask {
    id,
    number: NonZeroU32::new(number).expect("number"),
    revision: TaskRevision::FIRST,
    title: name(title),
    brief: String::new(),
    criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
    status: TaskStatus::Draft,
    organization: TaskOrganization::default(),
    module: None,
    former_key: former.map(str::to_owned),
    created_by: "Alice Moreau".to_owned(),
    created_at: at(50),
    updated_by: "Alice Moreau".to_owned(),
    updated_at: at(60),
    claim: None,
    run: None,
  }
}

/// Kontur joins ORC, whose tasks are ORC-1 … ORC-3 (numbers its own tasks have too), uploading
/// KON-2 as ORC-4; KON-1 and KON-3 stay Not shared.
fn joined() -> (TempDir, ProjectDb, Vec<Task>, Tag) {
  let (repo, mut db, tasks, _) = kontur();
  let server_tag = Tag::new(TagId::generate(), name("desktop"));
  let mut server: Vec<CachedTask> = (1..=3)
    .map(|number| server_task(TaskId::generate(), number, "Alice's task", None))
    .collect();
  server.push(server_task(tasks[1].id(), 4, "Fix reconnect", Some("KON-2")));
  let claimed = HashMap::from([(tasks[1].id(), NonZeroU32::new(4).expect("run"))]);
  let uploaded_read = db.task(tasks[1].id()).expect("read").expect("task").revision();
  let changed = db
    .join_sharing(
      &link(),
      (std::slice::from_ref(&server_tag), &server),
      (&claimed, &HashMap::from([(tasks[1].id(), uploaded_read)])),
    )
    .expect("join");
  assert!(changed.is_empty(), "nothing changed while joining");
  (repo, db, tasks, server_tag)
}

#[test]
fn joined_ticked_tasks_take_new_numbers_and_unticked_ones_stay_not_shared() {
  let (repo, db, tasks, _) = joined();
  assert_eq!(db.project().key().as_str(), "ORC");
  let listed: Vec<String> = db
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  assert_eq!(
    listed,
    ["ORC-1", "ORC-2", "ORC-3", "ORC-4"],
    "Not shared tasks are out of the list"
  );
  let kept: Vec<String> = db
    .not_shared_tasks()
    .expect("kept")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  assert_eq!(kept, ["KON-1", "KON-3"]);
  let page = db
    .task(tasks[0].id())
    .expect("read")
    .expect("a kept task stays readable");
  assert_eq!(page.key().to_string(), "KON-1");
  assert!(
    page.organization().tags().is_empty(),
    "the project's tags are the server's now"
  );
  assert_eq!(page.organization().priority(), Priority::Low);
  let info = db.shared_task_info(tasks[0].id()).expect("info").expect("kept");
  assert_eq!(info.kept_tags, Some(vec!["CI".to_owned(), "Desktop".to_owned()]));
  assert_eq!(
    db.shared_task_info(tasks[1].id())
      .expect("info")
      .expect("shared")
      .kept_tags,
    None
  );
  assert_eq!(
    db.former_keys().expect("former"),
    HashMap::from([(tasks[1].id(), "KON-2".to_owned())])
  );

  let mapping: Vec<(String, String, bool, Option<u32>)> = db
    .joined_keys()
    .expect("mapping")
    .into_iter()
    .map(|key| {
      (
        key.former,
        key.key.to_string(),
        key.not_shared,
        key.claimed_run.map(NonZeroU32::get),
      )
    })
    .collect();
  assert_eq!(
    mapping,
    [
      ("KON-2".to_owned(), "ORC-4".to_owned(), false, Some(4)),
      ("KON-1".to_owned(), "KON-1".to_owned(), true, None),
      ("KON-3".to_owned(), "KON-3".to_owned(), true, None),
    ]
  );
  drop(db);
  assert_eq!(
    ProjectDb::open(repo.path()).expect("open").project().key().as_str(),
    "ORC",
    "the key is the server project's when opened again"
  );
}

#[test]
fn joined_not_shared_tasks_are_read_only() {
  let (_repo, mut db, tasks, _) = joined();
  let kept = db.task(tasks[2].id()).expect("read").expect("task");
  assert!(matches!(
    db.update_task(kept.id(), kept.revision(), details("Edited")),
    Err(StoreError::NotSharedTask(id)) if id == kept.id()
  ));
  assert!(matches!(
    db.set_task_organization(
      kept.id(),
      &TaskOrganization::default().with_priority(Priority::High),
      ChangeActor::Person,
      at(200)
    ),
    Err(StoreError::NotSharedTask(_))
  ));
  assert!(matches!(
    db.join_sharing(&link(), (&[], &[]), (&HashMap::new(), &HashMap::new())),
    Err(StoreError::AlreadyShared)
  ));
}

#[test]
fn joined_share_task_makes_a_kept_task_the_server_s() {
  let (_repo, mut db, tasks, _) = joined();
  db.cache_shared_tasks(
    &[server_task(tasks[2].id(), 5, "Retry the flaky test", Some("KON-3"))],
    None,
  )
  .expect("shared later");
  let keys: Vec<String> = db
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  assert_eq!(keys, ["ORC-1", "ORC-2", "ORC-3", "ORC-4", "ORC-5"]);
  assert_eq!(db.not_shared_tasks().expect("kept").len(), 1);
  let mapping = db.joined_keys().expect("mapping");
  let shared = mapping
    .iter()
    .find(|key| key.task == tasks[2].id())
    .expect("in the mapping");
  assert_eq!((shared.key.to_string().as_str(), shared.not_shared), ("ORC-5", false));
}

#[test]
fn joined_make_local_returns_not_shared_tasks_after_the_highest_number() {
  let (_repo, mut db, tasks, server_tag) = joined();
  let returned = db.make_local().expect("make local");
  let mapping: Vec<(String, String)> = returned
    .iter()
    .map(|task| (task.before.to_string(), task.after.to_string()))
    .collect();
  assert_eq!(
    mapping,
    [
      ("KON-1".to_owned(), "ORC-5".to_owned()),
      ("KON-3".to_owned(), "ORC-6".to_owned())
    ]
  );
  let keys: Vec<String> = db
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  assert_eq!(keys, ["ORC-1", "ORC-2", "ORC-3", "ORC-4", "ORC-5", "ORC-6"]);
  assert!(db.not_shared_tasks().expect("kept").is_empty());
  assert!(db.joined_keys().expect("mapping").is_empty());
  assert_eq!(
    db.former_keys()
      .expect("former")
      .get(&tasks[0].id())
      .map(String::as_str),
    Some("KON-1")
  );
  // Tags come back by name: Desktop matches the server's "desktop", CI is made again.
  let tags = db.project_tags().expect("tags");
  let first = db.task(tasks[0].id()).expect("read").expect("task");
  let mut names: Vec<&str> = first
    .organization()
    .tags()
    .iter()
    .map(|id| {
      tags
        .iter()
        .find(|tag| tag.id() == *id)
        .expect("a project tag")
        .name()
        .as_str()
    })
    .collect();
  names.sort_unstable();
  assert_eq!(names, ["CI", "desktop"]);
  assert!(first.organization().tags().contains(&server_tag.id()));
  let next = db
    .create_tasks(IdempotencyKey::generate(), vec![details("After")], at(300))
    .expect("a local task")
    .remove(0);
  assert_eq!(next.key().to_string(), "ORC-7");
  db.update_task(first.id(), first.revision(), details("Edited here"))
    .expect("edit")
    .saved()
    .expect("a returned task changes here");
}

/// A populated database of schema 41 opens with its tasks, numbers and organization unchanged.
#[test]
fn joined_migration_keeps_a_populated_database_of_schema_41() {
  let (repo, db, tasks, _) = kontur();
  let before = db.tasks().expect("tasks");
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_JOINING);
  let mut db = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(db.tasks().expect("tasks"), before);
  assert!(db.joined_keys().expect("mapping").is_empty());
  let again = db
    .create_tasks(IdempotencyKey::generate(), vec![details("Fix reconnect")], at(30))
    .expect("numbers stay unique")
    .remove(0);
  assert_eq!(again.key().to_string(), "KON-4");
  assert_eq!(
    db.task(tasks[0].id()).expect("read").expect("task").key().to_string(),
    "KON-1"
  );
}

/// A change made here while the project was joining (a run's) is reported, read in the same
/// transaction as the link, so it is sent on top of the uploaded copy instead of lost.
#[test]
fn joined_a_change_made_while_joining_is_reported() {
  let (_repo, mut db, tasks, _) = kontur();
  let read = db.task(tasks[1].id()).expect("read").expect("task");
  let edited = db
    .update_task(read.id(), read.revision(), details("Fix reconnect after sleep"))
    .expect("edit")
    .saved()
    .expect("saved");
  let uploaded = HashMap::from([(read.id(), read.revision())]);
  let changed = db
    .join_sharing(
      &link(),
      (&[], &[server_task(read.id(), 4, "Fix reconnect", Some("KON-2"))]),
      (&HashMap::new(), &uploaded),
    )
    .expect("join");
  assert_eq!(changed.len(), 1);
  assert_eq!(changed[0].details(), edited.details());
}
