//! Migration 158, team references: a database before it (a cached version naming a shared tool by its id) opens with
//! the tool read back as a team resource of kind tool. The references that named a connection and a secret by ids of
//! the publisher's computer are dropped by the migration that removed the bindings (`remove_bindings_migration`).
//! `cargo test -p orchester-store --test team_references_migration team_references_migration`.

mod support;

use std::collections::BTreeSet;

use orchester_core::{
  blob::ContentHash,
  id::{ResourceId, UserId},
  resource::{
    AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceKind, Slug, VersionNumber,
  },
  sharing::{SharedPerson, TaskRevision},
  store::ResourceStore,
};
use orchester_store::{CachedVersion, ProjectDb, ReferenceTarget, ResourceSharing};
use rusqlite::Connection;
use serde_json::json;
use support::{at, copied_at_version, name, started_run};

/// The schema before the references widened: `shared_version_references` allowed only a connection, an MCP server, a
/// secret or a tool. Migrations are append-only, so a later one still applies on open.
const BEFORE_TEAM_REFERENCES: u32 = 157;

fn instruction() -> Instruction {
  Instruction::new(
    PromptText::new("Use the publisher's connection.").expect("valid prompt"),
    AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()])),
    BTreeSet::from([InjectionEvent::Resume]),
    InstructionScope::Project,
  )
  .expect("valid instruction")
}

#[test]
fn team_references_migration_reads_a_shared_tool_as_a_team_tool() {
  let (repo, mut db, _run, _execution) = started_run();
  let id = db
    .create_resource(Slug::new("rules").expect("slug"), name("Rules"), &instruction(), at(10))
    .expect("create")
    .id();
  db.set_resource_sharing(
    id,
    Some(ResourceSharing::Shared(TaskRevision::try_from(1).expect("revision"))),
  )
  .expect("shared");
  let tool = ResourceId::generate();
  let version = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: at(20),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body: json!({ "schema": 1 }),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: Vec::new(),
    local_body: instruction(),
  };
  let stored = db.cache_shared_version(id, &version).expect("cache");
  drop(db);

  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_TEAM_REFERENCES);
  // What the old schema kept for a tool the version named: kind `tool`, which the new schema reads as `team_tool`.
  Connection::open(old.path().join(".orchester").join("orchester.db"))
    .expect("open the old database")
    .execute(
      "INSERT INTO shared_version_references (version_id, position, kind, target_id, name, role) \
       VALUES (?1, 2, 'tool', ?2, 'Deploy', NULL)",
      [stored.id().to_string(), tool.to_string()],
    )
    .expect("a shared tool reference of the old schema");

  let migrated = ProjectDb::open(old.path()).expect("migrate the existing project");
  let read = migrated
    .cached_version::<Instruction>(id, VersionNumber::FIRST)
    .expect("read")
    .expect("the version is kept");
  assert_eq!(
    read.references,
    [ReferenceTarget::TeamResource {
      kind: ResourceKind::Tool,
      id: tool
    }],
    "the tool is a team tool"
  );
}
