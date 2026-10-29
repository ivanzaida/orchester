//! The cache of a shared project's configuration: resources marked as the server's, versions kept with the server's
//! body and the local one, and this computer's bindings of their references.
//! `cargo test -p orchester-store --test shared_configuration shared_configuration`.

mod support;

use std::{collections::BTreeSet, path::Path};

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ResourceId, SecretId, UserId},
  resource::{
    AgentProfile, AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, Slug, VersionNumber,
  },
  sharing::{SharedPerson, TaskRevision},
  store::ResourceStore,
};
use orchester_store::{
  Bound, BoundBy, BoundReference, CachedVersion, ProjectDb, ReferenceBinding, ReferenceTarget, ResourceSharing,
  StoreError, VersionReference,
};
use rusqlite::Connection;
use serde_json::json;

use self::support::{at, copied_at_version, name, started_run};

fn instruction(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid prompt"),
    AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()])),
    BTreeSet::from([InjectionEvent::Resume]),
    InstructionScope::Project,
  )
  .expect("valid instruction")
}

fn publisher() -> SharedPerson {
  SharedPerson {
    id: UserId::generate(),
    name: "Olena".to_owned(),
  }
}

fn revision(value: u64) -> TaskRevision {
  TaskRevision::try_from(value).expect("a revision")
}

fn cached<B>(local_body: B, references: Vec<VersionReference>) -> CachedVersion<B> {
  CachedVersion {
    number: VersionNumber::FIRST,
    published_at: at(20),
    publisher: publisher(),
    server_body: json!({ "schema": 1, "body": { "text": "Use the publisher's connection." } }),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references,
    local_body,
  }
}

fn database(repo: &Path) -> Connection {
  Connection::open(repo.join(".orchester").join("orchester.db")).expect("open")
}

#[test]
fn shared_configuration_caches_a_version_with_both_bodies() {
  let (_repo, mut db, _run, _execution) = started_run();
  let id = db
    .create_resource(
      Slug::new("review-rules").expect("slug"),
      name("Review rules"),
      &instruction("Draft."),
      at(10),
    )
    .expect("create")
    .id();
  let references = vec![
    VersionReference {
      target: ReferenceTarget::Connection(ConnectionId::generate()),
      name: "Claude".to_owned(),
      role: Some("claude_cli".to_owned()),
    },
    VersionReference {
      target: ReferenceTarget::Secret(SecretId::generate()),
      name: "GitHub token".to_owned(),
      role: None,
    },
    VersionReference {
      target: ReferenceTarget::Tool(ResourceId::generate()),
      name: "Deploy".to_owned(),
      role: None,
    },
  ];
  let version = cached(instruction("Use the local connection."), references);

  let stored = db.cache_shared_version(id, &version).expect("cache");
  assert_eq!(stored.number(), VersionNumber::FIRST);
  assert_eq!(stored.body(), &version.local_body);
  assert_eq!(stored.published_at(), at(20));
  assert_eq!(
    db.latest_version::<Instruction>(id).expect("latest"),
    Some(stored.clone())
  );
  assert_eq!(
    db.cached_version::<Instruction>(id, VersionNumber::FIRST)
      .expect("read")
      .as_ref(),
    Some(&version)
  );

  // A binding changes the local body only; the server's stays as the server keeps it.
  let rebound = instruction("Use the other local connection.");
  assert!(
    db.rewrite_local_body(id, VersionNumber::FIRST, &rebound)
      .expect("rewrite")
  );
  let read = db
    .cached_version::<Instruction>(id, VersionNumber::FIRST)
    .expect("read")
    .expect("cached");
  assert_eq!(read.local_body, rebound);
  assert_eq!(read.server_body, version.server_body);
  assert_eq!(
    db.latest_version::<Instruction>(id)
      .expect("latest")
      .expect("a version")
      .body(),
    &rebound
  );
  assert_eq!(
    db.version::<Instruction>(id, VersionNumber::FIRST)
      .expect("version")
      .expect("v1")
      .id(),
    stored.id()
  );
}

#[test]
fn shared_configuration_keeps_a_version_published_here_and_names_its_publisher_once() {
  let (repo, mut db, _run, _execution) = started_run();
  let builder = db
    .resources(orchester_core::resource::ResourceKind::AgentProfile)
    .expect("profiles")[0]
    .id();
  let published = db.latest_version::<AgentProfile>(builder).expect("latest").expect("v1");
  assert_eq!(
    db.cached_version::<AgentProfile>(builder, VersionNumber::FIRST)
      .expect("read"),
    None,
    "a version published here is not cached until the server confirms it"
  );

  let version = cached(published.body().clone(), Vec::new());
  let stored = db.cache_shared_version(builder, &version).expect("cache");
  assert_eq!(stored.id(), published.id());
  assert_eq!(stored.published_at(), published.published_at());
  let read = db
    .cached_version::<AgentProfile>(builder, VersionNumber::FIRST)
    .expect("read")
    .expect("cached");
  assert_eq!(read.publisher, version.publisher);
  assert!(read.references.is_empty());

  // Its publisher is set once: caching it again keeps the first, and a version stays immutable.
  let again = CachedVersion {
    publisher: publisher(),
    ..version.clone()
  };
  db.cache_shared_version(builder, &again).expect("cache again");
  let read = db
    .cached_version::<AgentProfile>(builder, VersionNumber::FIRST)
    .expect("read")
    .expect("cached");
  assert_eq!(read.publisher, version.publisher);
  let raw = database(repo.path());
  assert!(
    raw
      .execute("UPDATE resource_versions SET publisher_name = 'Taras'", [])
      .is_err()
  );
  assert!(raw.execute("UPDATE resource_versions SET body = '{}'", []).is_err());
}

#[test]
fn shared_configuration_sets_and_changes_a_binding() {
  let (_repo, mut db, _run, _execution) = started_run();
  assert!(db.reference_bindings().expect("bindings").is_empty());
  let reference = ConnectionId::generate();
  let unbound = ReferenceBinding {
    reference: BoundReference::McpServer { reference, bound: None },
    name: "GitHub".to_owned(),
    role: None,
    changed_at: at(30),
  };
  db.set_reference_binding(&unbound).expect("unbound");
  assert_eq!(db.reference_bindings().expect("bindings"), vec![unbound.clone()]);

  let local = ConnectionId::generate();
  let bound = ReferenceBinding {
    reference: BoundReference::McpServer {
      reference,
      bound: Some(Bound {
        local,
        by: BoundBy::Person,
      }),
    },
    changed_at: at(31),
    ..unbound.clone()
  };
  db.set_reference_binding(&bound).expect("bound");
  assert_eq!(db.reference_bindings().expect("bindings"), vec![bound]);

  let secret = ReferenceBinding {
    reference: BoundReference::Secret {
      reference: SecretId::generate(),
      bound: Some(Bound {
        local: SecretId::generate(),
        by: BoundBy::Name,
      }),
    },
    name: "Deploy token".to_owned(),
    role: Some("deployment_token".to_owned()),
    changed_at: at(32),
  };
  db.set_reference_binding(&secret).expect("secret");
  let rebound = ReferenceBinding {
    reference: BoundReference::McpServer {
      reference,
      bound: Some(Bound {
        local: ConnectionId::generate(),
        by: BoundBy::Role,
      }),
    },
    changed_at: at(33),
    ..unbound
  };
  db.set_reference_binding(&rebound).expect("rebound");
  assert_eq!(db.reference_bindings().expect("bindings"), vec![rebound, secret]);
}

#[test]
fn shared_configuration_marks_a_resource_shared() {
  let (repo, mut db, _run, _execution) = started_run();
  let builder = db
    .resources(orchester_core::resource::ResourceKind::AgentProfile)
    .expect("profiles")[0]
    .id();
  assert_eq!(db.resource_sharing(builder).expect("local"), None);

  db.set_resource_sharing(builder, Some(ResourceSharing::Shared(revision(3))))
    .expect("shared");
  assert_eq!(
    db.resource_sharing(builder).expect("shared"),
    Some(ResourceSharing::Shared(revision(3)))
  );
  db.set_resource_sharing(builder, Some(ResourceSharing::NotShared))
    .expect("not shared");
  assert_eq!(
    db.resource_sharing(builder).expect("not shared"),
    Some(ResourceSharing::NotShared)
  );
  db.set_resource_sharing(builder, None).expect("local again");
  assert_eq!(db.resource_sharing(builder).expect("local"), None);

  let missing = ResourceId::generate();
  assert!(matches!(
    db.set_resource_sharing(missing, Some(ResourceSharing::NotShared)),
    Err(StoreError::ResourceNotFound(id)) if id == missing
  ));
  let raw = database(repo.path());
  assert!(
    raw
      .execute("UPDATE resources SET sharing = 'shared', server_revision = NULL", [])
      .is_err(),
    "a shared resource has its server revision"
  );
}

#[test]
fn shared_configuration_migration_runs_forward() {
  let (repo, db, _run, _execution) = started_run();
  let builder = db
    .resources(orchester_core::resource::ResourceKind::AgentProfile)
    .expect("profiles")[0]
    .id();
  let published = db.latest_version::<AgentProfile>(builder).expect("latest").expect("v1");
  let current: u32 = database(repo.path())
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version");
  drop(db);

  // Before the shared configuration's migration: only the run's switch to Manual (ORC-353) came after it.
  let old = copied_at_version(&repo.path().join(".orchester").join("orchester.db"), current - 2);
  let mut db = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(db.resource_sharing(builder).expect("local"), None);
  assert_eq!(
    db.latest_version::<AgentProfile>(builder).expect("latest"),
    Some(published.clone())
  );
  assert!(db.reference_bindings().expect("bindings").is_empty());
  db.set_resource_sharing(builder, Some(ResourceSharing::Shared(revision(1))))
    .expect("shared");
  db.cache_shared_version(builder, &cached(published.body().clone(), Vec::new()))
    .expect("cache");
  assert!(
    database(old.path())
      .execute("DELETE FROM resource_versions", [])
      .is_err()
  );
}
