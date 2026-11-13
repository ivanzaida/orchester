//! The cache of a shared project's configuration: resources marked as the server's, and versions kept with the server's
//! body and the local one, with the references they name.
//! `cargo test -p orchester-store --test shared_configuration shared_configuration`.

mod support;

use std::{collections::BTreeSet, path::Path};

use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{ResourceId, UserId},
  resource::{
    AgentProfile, AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceKind, Slug,
    VersionNumber,
  },
  secret::SecretName,
  sharing::{SharedPerson, TaskRevision},
  store::ResourceStore,
};
use orchester_store::{CachedVersion, ProjectDb, ReferenceTarget, ResourceSharing, StoreError};
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

fn cached<B>(local_body: B, references: Vec<ReferenceTarget>) -> CachedVersion<B> {
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

/// A reference of each kind a version names: a CLI kind, team resources by server id and a secret by name.
fn named_references() -> Vec<ReferenceTarget> {
  vec![
    ReferenceTarget::CliKind(AgentProvider::CodexCli),
    ReferenceTarget::TeamResource {
      kind: ResourceKind::McpServer,
      id: ResourceId::generate(),
    },
    ReferenceTarget::TeamResource {
      kind: ResourceKind::Tool,
      id: ResourceId::generate(),
    },
    ReferenceTarget::SecretName(SecretName::new("STAGING_TOKEN").expect("a secret name")),
  ]
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
  let references = named_references();
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

  // A team profile's connection pick changes the local body only; the server's stays as the server keeps it.
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

/// A team resource is identified by its server id, so it arrives with the slug the server gave it whether or not one of
/// this computer's own has it, and the computer's own stays as it is (`docs/design/team-configuration.md`, edge cases 2
/// and 3). Two live team resources of one slug are the server's to sort out.
#[test]
fn shared_configuration_a_team_resource_keeps_its_slug_beside_a_local_one() {
  let (_repo, mut db, run, _execution) = started_run();
  let body = db
    .latest_version::<orchester_core::resource::Pipeline>(run.snapshot().pipeline().resource)
    .expect("read")
    .expect("the run's pipeline")
    .body()
    .clone();
  // A pipeline is created as its first version (D-032).
  let (own, _) = db
    .published_pipeline_for_test("review", "Review", &body, at(10))
    .expect("a local pipeline");
  let team_resource = |kind, slug: &str, at_ms| {
    orchester_core::resource::Resource::new(
      ResourceId::generate(),
      kind,
      Slug::new(slug).expect("slug"),
      name("Review"),
      at(at_ms),
    )
  };
  let slug = |db: &ProjectDb, id| db.resource(id).expect("read").expect("kept").slug().as_str().to_owned();

  let team = team_resource(ResourceKind::Pipeline, "review", 20);
  db.keep_shared_resource(&team, revision(1)).expect("the team's arrives");
  assert_eq!(
    (slug(&db, team.id()), slug(&db, own)),
    ("review".into(), "review".into())
  );
  assert_eq!(
    db.resource_sharing(own).expect("read"),
    None,
    "still this computer's own, untouched"
  );
  assert_eq!(
    db.resource_sharing(team.id()).expect("read"),
    Some(ResourceSharing::Shared(revision(1)))
  );

  // Every kind arrives the same way.
  let profile = db.resources(ResourceKind::AgentProfile).expect("profiles").remove(0);
  let team_profile = orchester_core::resource::Resource::new(
    ResourceId::generate(),
    profile.kind(),
    profile.slug().clone(),
    name("Builder"),
    at(30),
  );
  db.keep_shared_resource(&team_profile, revision(1))
    .expect("the team's arrives");
  assert_eq!(slug(&db, team_profile.id()), profile.slug().as_str());
  assert_eq!(slug(&db, profile.id()), profile.slug().as_str());

  // A team resource restored on the server under a new id arrives while the archived one is kept here.
  db.mark_shared_archived(team.id(), true, at(40)).expect("archived");
  let restored = team_resource(ResourceKind::Pipeline, "review", 50);
  db.keep_shared_resource(&restored, revision(1))
    .expect("the restored one arrives");
  assert_eq!(slug(&db, restored.id()), "review");
  assert_eq!(slug(&db, team.id()), "review");

  // Two live team resources of one slug are the server's to sort out: refused.
  let other = team_resource(ResourceKind::Pipeline, "review", 60);
  assert!(matches!(
    db.keep_shared_resource(&other, revision(1)),
    Err(StoreError::DuplicateSlug { .. })
  ));
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

  // Before shared configuration's migration, which ORC-346's request answers, the run's switch to Manual (ORC-353)
  // and ORC-387's waiting versions follow.
  let old = copied_at_version(&repo.path().join(".orchester").join("orchester.db"), current - 4);
  let mut db = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(db.resource_sharing(builder).expect("local"), None);
  assert_eq!(
    db.latest_version::<AgentProfile>(builder).expect("latest"),
    Some(published.clone())
  );
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
