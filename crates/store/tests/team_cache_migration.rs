//! The migration that drops a shared project's copy of the team's configuration
//! (`docs/design/team-configuration-live.md`): a shared project's database before it, with the team's resources, their
//! versions, bodies and references cached, opens with every team version gone except the one a task here pins (with its
//! resource and bodies), a team resource left with no version gone, the person's own resources and what this computer
//! filled in untouched, and the published versions as immutable as before; applying it again changes nothing, and a
//! local project is not touched. `cargo test -p orchester-store --test team_cache_migration team_cache_migration`.

mod support;

use std::fs;

use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId, TaskId},
  project::{Project, ProjectKey},
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, Resource, Slug},
  sharing::TaskRevision,
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
};
use orchester_store::{ConnectionPick, ProjectDb, ProjectMigration, ResourceSharing};
use rusqlite::Connection;
use support::{at, copied_at_version, name};
use tempfile::TempDir;

fn project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("project");
  (repo, db)
}

fn rule(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("prompt"),
    AgentTargets::All,
    [InjectionEvent::Start].into(),
    InstructionScope::Project,
  )
  .expect("instruction")
}

/// An instruction published as v1 here, and as v2 when `twice`.
fn published(db: &mut ProjectDb, slug: &str, twice: bool) -> Resource {
  let resource = db
    .create_resource(Slug::new(slug).expect("slug"), name(slug), &rule("Use tabs."), at(1))
    .expect("create");
  db.publish_draft::<Instruction>(resource.id(), at(2)).expect("v1");
  if twice {
    db.save_draft(
      resource.id(),
      &orchester_core::resource::Draft::new(rule("Use spaces."), Some(number(1)), at(3)),
    )
    .expect("a draft");
    db.publish_draft::<Instruction>(resource.id(), at(4)).expect("v2");
  }
  resource
}

fn number(value: u32) -> orchester_core::resource::VersionNumber {
  orchester_core::resource::VersionNumber::new(value.try_into().expect("above zero"))
}

/// What the database held before the migration.
struct Seeded {
  /// The team's resource with v1 (pinned by `task`) and v2 (pinned by nothing).
  pinned: ResourceId,
  task: TaskId,
  /// The team's resource with v1 only, which nothing pins.
  unpinned: ResourceId,
  /// The person's own resource, kept *Not shared*.
  own: ResourceId,
  picked: ResourceId,
}

fn seeded(db: &mut ProjectDb) -> Seeded {
  let revision = TaskRevision::try_from(1).expect("a revision");
  let pinned = published(db, "style", true);
  db.keep_shared_resource(&pinned, revision).expect("the team's");
  let unpinned = published(db, "naming", false);
  db.keep_shared_resource(&unpinned, revision).expect("the team's");
  let own = published(db, "mine", false);
  db.set_resource_sharing(own.id(), Some(ResourceSharing::NotShared))
    .expect("kept as the person's own");
  let details = TaskDetails {
    title: name("Ship it"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let task = db
    .create_tasks(orchester_core::id::IdempotencyKey::generate(), vec![details], at(5))
    .expect("a task")
    .remove(0)
    .id();
  let picked = ResourceId::generate();
  db.set_connection_pick(
    picked,
    ConnectionPick {
      connection: ConnectionId::generate(),
      account: None,
    },
  )
  .expect("a pick");
  Seeded {
    pinned: pinned.id(),
    task,
    unpinned: unpinned.id(),
    own: own.id(),
    picked,
  }
}

/// The database rewound to before the migration with the cache's rows, opened again so it applies; `shared` makes it a
/// shared project's.
fn migrated(repo: &TempDir, seeded: &Seeded, shared: bool) -> (TempDir, ProjectDb) {
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    ProjectDb::version_before(ProjectMigration::TeamCache),
  );
  let connection = Connection::open(old.path().join(".orchester").join("orchester.db")).expect("the old database");
  let sharing = format!(
    "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, last_synced_at) \
     VALUES (1, '{}', '{}', 'KON', 'owner', 1, 0, 1);",
    ProjectId::generate(),
    ProjectId::generate(),
  );
  connection
    .execute_batch(&format!(
      "{sharing}
       INSERT INTO shared_version_bodies (version_id, server_body, sha256, local_body) \
         SELECT id, body, '{hash}', body FROM resource_versions WHERE resource_id IN ('{pinned}', '{unpinned}');
       INSERT INTO shared_version_references (version_id, position, kind, target_id) \
         SELECT id, 0, 'cli_kind', 'claude' FROM resource_versions WHERE resource_id IN ('{pinned}', '{unpinned}');
       UPDATE tasks SET pipeline_id = '{pinned}', pipeline_version = 1 WHERE id = '{task}';",
      sharing = if shared { sharing.as_str() } else { "" },
      hash = "a".repeat(64),
      pinned = seeded.pinned,
      unpinned = seeded.unpinned,
      task = seeded.task,
    ))
    .expect("the cache's rows");
  drop(connection);
  let db = ProjectDb::open(old.path()).expect("migrated");
  (old, db)
}

fn count(path: &std::path::Path, sql: &str) -> i64 {
  Connection::open(path)
    .expect("open")
    .query_row(sql, [], |row| row.get(0))
    .expect("count")
}

#[test]
fn team_cache_migration_keeps_only_what_a_task_pins_and_what_this_computer_filled_in() {
  let (repo, mut db) = project();
  let seeded = seeded(&mut db);
  drop(db);
  let (old, db) = migrated(&repo, &seeded, true);
  assert!(
    db.connection_pick(seeded.picked).expect("read").is_some(),
    "the overrides stay"
  );
  drop(db);
  let path = old.path().join(".orchester").join("orchester.db");
  let versions = |resource: ResourceId| {
    count(
      &path,
      &format!("SELECT COUNT(*) FROM resource_versions WHERE resource_id = '{resource}'"),
    )
  };
  assert_eq!(versions(seeded.pinned), 1, "only the version the task pins stays");
  assert_eq!(
    count(
      &path,
      &format!(
        "SELECT number FROM resource_versions WHERE resource_id = '{}'",
        seeded.pinned
      )
    ),
    1
  );
  assert_eq!(
    count(&path, "SELECT COUNT(*) FROM shared_version_bodies"),
    1,
    "the pinned version keeps its bodies"
  );
  assert_eq!(count(&path, "SELECT COUNT(*) FROM shared_version_references"), 1);
  assert_eq!(versions(seeded.unpinned), 0, "a version nothing pins goes");
  assert_eq!(
    count(
      &path,
      &format!("SELECT COUNT(*) FROM resources WHERE id = '{}'", seeded.unpinned)
    ),
    0,
    "a team resource with no version left goes"
  );
  assert_eq!(versions(seeded.own), 1, "the person's own resource stays");
  assert_eq!(
    count(
      &path,
      &format!("SELECT COUNT(*) FROM tasks WHERE pipeline_id = '{}'", seeded.pinned)
    ),
    1,
    "the task keeps the pipeline it pins"
  );

  let connection = Connection::open(&path).expect("open");
  let deleted = connection.execute("DELETE FROM resource_versions", []);
  assert!(deleted.is_err(), "a published version stays");
}

#[test]
fn team_cache_migration_applied_again_changes_nothing() {
  let (repo, mut db) = project();
  let seeded = seeded(&mut db);
  drop(db);
  let (old, db) = migrated(&repo, &seeded, true);
  drop(db);
  let path = old.path().join(".orchester").join("orchester.db");
  let connection = Connection::open(&path).expect("open");

  ProjectDb::apply_migration_for_test(&connection, ProjectMigration::TeamCache).expect("applied again");
  drop(connection);

  assert_eq!(count(&path, "SELECT COUNT(*) FROM resource_versions"), 2);
  assert_eq!(count(&path, "SELECT COUNT(*) FROM shared_version_bodies"), 1);
}

#[test]
fn team_cache_migration_leaves_a_local_project_as_it_is() {
  let (repo, mut db) = project();
  let seeded = seeded(&mut db);
  drop(db);
  let (old, db) = migrated(&repo, &seeded, false);
  drop(db);
  let path = old.path().join(".orchester").join("orchester.db");

  assert_eq!(count(&path, "SELECT COUNT(*) FROM resource_versions"), 4);
  assert_eq!(count(&path, "SELECT COUNT(*) FROM shared_version_bodies"), 3);
}
