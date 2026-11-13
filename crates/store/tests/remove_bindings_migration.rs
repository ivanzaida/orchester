//! Migration 163, the bindings removed: a database before it opens without the reference bindings, without the
//! references that named a connection, an MCP server or a secret by an id of the publisher's computer (the team's are
//! kept), and with a team resource's slug free of the slug of one of this computer's own.
//! `cargo test -p orchester-store --test remove_bindings_migration remove_bindings_migration`.

mod support;

use std::collections::BTreeSet;

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ResourceId, SecretId, UserId},
  resource::{
    AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, Resource, ResourceKind, Slug,
    VersionNumber,
  },
  sharing::{SharedPerson, TaskRevision},
  store::ResourceStore,
};
use orchester_store::{CachedVersion, ProjectDb, ReferenceTarget, ResourceSharing};
use rusqlite::Connection;
use serde_json::json;
use support::{at, copied_at_version, name, started_run};

/// The schema before the bindings went: `reference_bindings` still there, and `resources` unique by kind and slug.
/// Migrations are append-only, so later ones still apply on open.
const BEFORE_REMOVE_BINDINGS: u32 = 162;

fn instruction() -> Instruction {
  Instruction::new(
    PromptText::new("Use the team's way.").expect("valid prompt"),
    AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()])),
    BTreeSet::from([InjectionEvent::Resume]),
    InstructionScope::Project,
  )
  .expect("valid instruction")
}

fn old_database(repo: &std::path::Path) -> tempfile::TempDir {
  copied_at_version(&repo.join(".orchester").join("orchester.db"), BEFORE_REMOVE_BINDINGS)
}

fn tables(database: &std::path::Path) -> Vec<String> {
  let raw = Connection::open(database).expect("open");
  let mut statement = raw
    .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
    .expect("tables");
  statement
    .query_map([], |row| row.get(0))
    .expect("names")
    .collect::<Result<_, _>>()
    .expect("rows")
}

#[test]
fn remove_bindings_migration_drops_the_bindings_and_the_references_that_named_a_computers_ids() {
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
  let team_tool = ReferenceTarget::TeamResource {
    kind: ResourceKind::Tool,
    id: ResourceId::generate(),
  };
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

  let old = old_database(repo.path());
  let file = old.path().join(".orchester").join("orchester.db");
  assert!(tables(&file).contains(&"reference_bindings".to_owned()));
  {
    let raw = Connection::open(&file).expect("the old database");
    let legacy = |position: i64, kind: &str, target: String, role: Option<&str>| {
      raw
        .execute(
          "INSERT INTO shared_version_references (version_id, position, kind, target_id, name, role) \
           VALUES (?1, ?2, ?3, ?4, 'Named', ?5)",
          rusqlite::params![stored.id().to_string(), position, kind, target, role],
        )
        .expect("a reference of the old model");
    };
    // The old schema names each reference, so the team's too is written the way it was then.
    let ReferenceTarget::TeamResource { id: team_id, .. } = &team_tool else {
      unreachable!("the reference is a team resource");
    };
    legacy(0, "team_tool", team_id.to_string(), None);
    legacy(
      1,
      "connection",
      ConnectionId::generate().to_string(),
      Some("claude_code_cli"),
    );
    legacy(2, "mcp_server", ConnectionId::generate().to_string(), None);
    legacy(3, "secret", SecretId::generate().to_string(), Some("api_key"));
    raw
      .execute(
        "INSERT INTO reference_bindings (kind, reference_id, name, changed_at) VALUES ('secret', ?1, 'Token', 1)",
        [SecretId::generate().to_string()],
      )
      .expect("a binding of the old model");
  }

  let migrated = ProjectDb::open(old.path()).expect("migrate the existing project");
  let read = migrated
    .cached_version::<Instruction>(id, VersionNumber::FIRST)
    .expect("read")
    .expect("the version is kept");
  assert_eq!(read.references, [team_tool], "only the team's reference stays");
  drop(migrated);
  assert!(!tables(&file).contains(&"reference_bindings".to_owned()));
}

#[test]
fn remove_bindings_migration_lets_a_team_resource_keep_its_slug_beside_a_local_one() {
  let (repo, mut db, run, _execution) = started_run();
  let body = db
    .latest_version::<orchester_core::resource::Pipeline>(run.snapshot().pipeline().resource)
    .expect("read")
    .expect("the run's pipeline")
    .body()
    .clone();
  // A pipeline is created as its first version (D-032).
  db.published_pipeline_for_test("review", "Review", &body, at(10))
    .expect("a local pipeline");
  drop(db);

  let old = old_database(repo.path());
  let raw = Connection::open(old.path().join(".orchester").join("orchester.db")).expect("the old database");
  let duplicate = "INSERT INTO resources (id, kind, slug, name, created_at, sharing, server_revision) \
                   VALUES (?1, 'pipeline', 'review', 'Review', 1, 'shared', 1)";
  assert!(
    raw.execute(duplicate, [ResourceId::generate().to_string()]).is_err(),
    "before the migration a slug is unique for its kind"
  );
  drop(raw);

  let mut migrated = ProjectDb::open(old.path()).expect("migrate the existing project");
  let team = Resource::new(
    ResourceId::generate(),
    ResourceKind::Pipeline,
    Slug::new("review").expect("slug"),
    name("Review"),
    at(20),
  );
  migrated
    .keep_shared_resource(&team, TaskRevision::try_from(1).expect("revision"))
    .expect("the team's pipeline arrives with its slug");
  let slugs: Vec<_> = migrated
    .kept_resources(ResourceKind::Pipeline)
    .expect("pipelines")
    .iter()
    .map(|resource| resource.slug().as_str().to_owned())
    .filter(|slug| slug == "review")
    .collect();
  assert_eq!(slugs, ["review", "review"]);
}
