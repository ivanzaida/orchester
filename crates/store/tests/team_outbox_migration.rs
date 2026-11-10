//! The migration that drops the outbox of the team's configuration (`docs/design/team-configuration-live.md`, rule 6):
//! a shared project's database before it, with a version published here that the server never numbered, a teammate's
//! version waiting for *Use v<N> here* and a draft of a team resource, opens with the unsent version and the draft
//! gone, the versions the server numbered and the person's own drafts kept, the overrides this computer filled in
//! untouched, the published versions as immutable as before, and no outbox table; applying it again changes nothing.
//! `cargo test -p orchester-store --test team_outbox_migration team_outbox_migration`.

mod support;

use std::fs;

use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId},
  project::{Project, ProjectKey},
  resource::{AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, Resource, Slug},
  sharing::TaskRevision,
  store::ResourceStore,
};
use orchester_store::{ConnectionPick, ProjectDb, ProjectMigration, ResourceSharing, StoreError};
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

/// An instruction published as v1 here.
fn published(db: &mut ProjectDb, slug: &str, text: &str) -> Resource {
  let resource = db
    .create_resource(Slug::new(slug).expect("slug"), name(slug), &rule(text), at(1))
    .expect("create");
  db.publish_draft::<Instruction>(resource.id(), at(2)).expect("v1");
  resource
}

/// What the database held before the migration.
struct Seeded {
  /// The team's instruction: v1 numbered by the server, v2 published here and never sent, and a draft.
  team: ResourceId,
  numbered: String,
  unsent: String,
  /// The person's own instruction, kept *Not shared*, with a draft.
  own: ResourceId,
  picked: ResourceId,
}

/// The old shape: the rows a shared project kept before the migration, outbox included.
fn seeded(db: &mut ProjectDb) -> Seeded {
  let team = published(db, "style", "Use tabs.");
  db.keep_shared_resource(&team, TaskRevision::try_from(1).expect("a revision"))
    .expect("the team's");
  let numbered = db
    .versions::<Instruction>(team.id())
    .expect("v1")
    .remove(0)
    .id()
    .to_string();
  db.save_draft(team.id(), &Draft::new(rule("Use spaces."), Some(number(1)), at(3)))
    .expect("a draft");
  let unsent = db.publish_draft::<Instruction>(team.id(), at(4)).expect("v2 here");
  db.save_draft(team.id(), &Draft::new(rule("Use both."), Some(number(2)), at(5)))
    .expect("a draft of the team's");
  let own = published(db, "mine", "Mine.");
  db.set_resource_sharing(own.id(), Some(ResourceSharing::NotShared))
    .expect("kept as the person's own");
  db.save_draft(own.id(), &Draft::new(rule("Still mine."), Some(number(1)), at(6)))
    .expect("the person's own draft");
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
    team: team.id(),
    numbered,
    unsent: unsent.id().to_string(),
    own: own.id(),
    picked,
  }
}

fn number(value: u32) -> orchester_core::resource::VersionNumber {
  orchester_core::resource::VersionNumber::new(value.try_into().expect("above zero"))
}

/// The database rewound to before the migration with the outbox's rows, opened again so it applies.
fn migrated(repo: &TempDir, seeded: &Seeded) -> (TempDir, ProjectDb) {
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    ProjectDb::version_before(ProjectMigration::TeamOutbox),
  );
  let connection = Connection::open(old.path().join(".orchester").join("orchester.db")).expect("the old database");
  connection
    .execute_batch(&format!(
      "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, last_synced_at) \
       VALUES (1, '{server}', '{project}', 'KON', 'owner', 1, 0, 1);
       INSERT INTO shared_publishes (version_id, resource_id, idempotency_key) \
       VALUES ('{unsent}', '{team}', 'key-of-the-unsent-version');
       INSERT INTO shared_version_bodies (version_id, server_body, sha256, local_body) SELECT id, body, '{hash}', body FROM resource_versions WHERE id = '{numbered}';
       INSERT INTO waiting_versions (version_id) VALUES ('{numbered}');",
      server = ProjectId::generate(),
      project = ProjectId::generate(),
      unsent = seeded.unsent,
      numbered = seeded.numbered,
      hash = "a".repeat(64),
      team = seeded.team,
    ))
    .expect("the outbox's rows");
  drop(connection);
  let db = ProjectDb::open(old.path()).expect("migrated");
  (old, db)
}

fn tables(db_path: &std::path::Path) -> Vec<String> {
  let connection = Connection::open(db_path).expect("open");
  let mut statement = connection
    .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
    .expect("tables");
  statement
    .query_map([], |row| row.get(0))
    .expect("tables")
    .collect::<Result<_, _>>()
    .expect("tables")
}

#[test]
fn team_outbox_migration_drops_the_unsent_version_and_the_team_drafts_and_keeps_the_rest() {
  let (repo, mut db) = project();
  let seeded = seeded(&mut db);
  drop(db);
  let (old, db) = migrated(&repo, &seeded);

  // The unsent version goes with the outbox, and the numbered one, which nothing here pins, with the team's cache
  // ().
  assert!(
    matches!(
      db.versions::<Instruction>(seeded.team),
      Err(StoreError::ResourceNotFound(_))
    ),
    "no version of the team's is kept here, nor the resource left with none"
  );
  assert!(
    !matches!(db.draft::<Instruction>(seeded.team), Ok(Some(_))),
    "no draft of a team resource"
  );
  assert!(
    db.draft::<Instruction>(seeded.own).expect("read").is_some(),
    "the person's own draft stays"
  );
  assert!(
    db.connection_pick(seeded.picked).expect("read").is_some(),
    "the overrides stay"
  );
  let path = old.path().join(".orchester").join("orchester.db");
  let tables = tables(&path);
  assert!(!tables.iter().any(|table| table == "shared_publishes"), "{tables:?}");
  assert!(!tables.iter().any(|table| table == "waiting_versions"), "{tables:?}");
  drop(db);

  let connection = Connection::open(&path).expect("open");
  let changed = connection.execute("UPDATE resource_versions SET number = 7", []);
  assert!(changed.is_err(), "a published version keeps its number");
  let deleted = connection.execute("DELETE FROM resource_versions", []);
  assert!(deleted.is_err(), "a published version stays");
}

#[test]
fn team_outbox_migration_applied_again_changes_nothing() {
  let (repo, mut db) = project();
  let seeded = seeded(&mut db);
  drop(db);
  let (old, db) = migrated(&repo, &seeded);
  drop(db);
  let path = old.path().join(".orchester").join("orchester.db");
  let connection = Connection::open(&path).expect("open");

  ProjectDb::apply_migration_for_test(&connection, ProjectMigration::TeamOutbox).expect("applied again");
  drop(connection);

  let db = ProjectDb::open(old.path()).expect("open");
  assert!(matches!(
    db.versions::<Instruction>(seeded.team),
    Err(StoreError::ResourceNotFound(_))
  ));
  assert!(db.draft::<Instruction>(seeded.own).expect("read").is_some());
  let tables = tables(&path);
  assert!(!tables.iter().any(|table| table == "shared_publishes"), "{tables:?}");
}
