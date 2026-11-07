//! Migration 150, team references: a database before it (a cached version naming a connection, a secret and a shared
//! tool by ids of the publisher's computer) opens with the connection and the secret kept as they were, and the tool
//! read back as a team resource of kind tool.
//! `cargo test -p orchester-store --test team_references_migration team_references_migration`.

mod support;

use std::collections::BTreeSet;

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ResourceId, SecretId, UserId},
  resource::{
    AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceKind, Slug, VersionNumber,
  },
  sharing::{SharedPerson, TaskRevision},
  store::ResourceStore,
};
use orchester_store::{CachedVersion, ProjectDb, ReferenceTarget, ResourceSharing, VersionReference};
use rusqlite::Connection;
use serde_json::json;
use support::{at, copied_at_version, name, started_run};

/// The schema before the references widened: `shared_version_references` allowed only a connection, an MCP server, a
/// secret or a tool. Migrations are append-only, so a later one still applies on open.
const BEFORE_TEAM_REFERENCES: u32 = 149;

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
fn team_references_migration_reads_a_shared_tool_as_a_team_tool_and_keeps_the_other_references() {
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
  let connection = ConnectionId::generate();
  let secret = SecretId::generate();
  let tool = ResourceId::generate();
  let reference = |target, name: &str, role: Option<&str>| VersionReference {
    target,
    name: name.to_owned(),
    role: role.map(str::to_owned),
  };
  let before = vec![
    reference(ReferenceTarget::Connection(connection), "Claude", Some("claude_code_cli")),
    reference(ReferenceTarget::Secret(secret), "GitHub token", None),
  ];
  let version = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: at(20),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body: json!({ "schema": 1 }),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: before.clone(),
    local_body: instruction(),
    waits_here: false,
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
  let mut expected = before;
  expected.push(reference(
    ReferenceTarget::TeamResource {
      kind: ResourceKind::Tool,
      id: tool,
    },
    "Deploy",
    None,
  ));
  assert_eq!(read.references, expected, "the tool is a team tool, the others as they were");
}
