//! A new resource is created as its first version, with no draft (`docs/design/team-configuration.md`, decision 6): a
//! body that cannot be published creates nothing, and a draft is only ever saved over a resource that holds a version
//! or a draft already.

use std::{collections::BTreeSet, fs};

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, PublishError, ResourceKind, Slug,
    TeamMcpServer, VersionNumber,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

fn project_repo() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn slug(value: &str) -> Slug {
  Slug::new(value).expect("valid slug")
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn instruction(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("valid instruction")
}

#[test]
fn created_resources_are_published_as_v1_with_no_draft() {
  let (_repo, mut db) = project_repo();

  let (resource, version) = db
    .create_published(slug("rule"), name("Rule"), &instruction("Be brief."), at(1))
    .expect("created");

  assert_eq!(version.number(), VersionNumber::FIRST);
  assert_eq!(db.versions::<Instruction>(resource.id()).unwrap().len(), 1);
  assert!(db.draft::<Instruction>(resource.id()).unwrap().is_none());
  assert_eq!(db.resources(ResourceKind::Instruction).unwrap().len(), 1);
}

#[test]
fn a_body_that_cannot_be_published_creates_nothing() {
  let (_repo, mut db) = project_repo();

  let refused = db.create_published(slug("blank"), name("Blank"), &instruction(""), at(1));

  assert!(matches!(
    refused,
    Err(StoreError::Publish(PublishError::BlankInstruction))
  ));
  assert!(db.resources(ResourceKind::Instruction).unwrap().is_empty());
  assert!(db.slugs(ResourceKind::Instruction).unwrap().is_empty());
}

#[test]
fn a_slug_taken_by_the_same_kind_creates_nothing() {
  let (_repo, mut db) = project_repo();
  db.create_published(slug("rule"), name("Rule"), &instruction("One."), at(1))
    .expect("created");

  let again = db.create_published(slug("rule"), name("Rule again"), &instruction("Two."), at(2));

  assert!(matches!(again, Err(StoreError::DuplicateSlug { .. })));
  assert_eq!(db.resources(ResourceKind::Instruction).unwrap().len(), 1);
}

#[test]
fn editing_a_created_resource_drafts_and_publishes_the_next_version() {
  let (_repo, mut db) = project_repo();
  let (resource, _) = db
    .create_published(slug("rule"), name("Rule"), &instruction("One."), at(1))
    .expect("created");

  db.save_draft(
    resource.id(),
    &Draft::new(instruction("Two."), Some(VersionNumber::FIRST), at(2)),
  )
  .expect("a draft over v1");
  let second = db
    .publish_draft::<Instruction>(resource.id(), at(3))
    .expect("published");

  assert_eq!(second.number().get(), 2);
  assert!(db.draft::<Instruction>(resource.id()).unwrap().is_none());
}

#[test]
fn a_draft_is_refused_for_a_resource_that_holds_nothing() {
  let (repo, mut db) = project_repo();
  let id = ResourceId::generate();
  rusqlite::Connection::open(repo.path().join(".orchester/orchester.db"))
    .expect("open raw database")
    .execute(
      "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, 'instruction', 'bare', 'Bare', 1)",
      [id.to_string()],
    )
    .expect("a resource without version or draft");

  let refused = db.save_draft(id, &Draft::new(instruction("Words."), None, at(2)));

  assert!(matches!(refused, Err(StoreError::NoFirstVersion(refused)) if refused == id));
  assert!(db.draft::<Instruction>(id).unwrap().is_none());
}

#[test]
fn only_a_version_the_server_has_not_numbered_is_taken_back() {
  let (_repo, mut db) = project_repo();
  let (resource, _) = db
    .create_published(slug("rule"), name("Rule"), &instruction("One."), at(1))
    .expect("created");

  // This project is local, so its v1 is final: no server is waiting to number it.
  let refused = db.discard_created(resource.id());

  assert!(
    matches!(refused, Err(StoreError::AlreadyNumbered(numbered)) if numbered == resource.id()),
    "{refused:?}"
  );
  assert_eq!(db.resources(ResourceKind::Instruction).unwrap().len(), 1);
}

#[test]
fn a_draft_is_refused_for_a_team_mcp_server_that_holds_nothing() {
  let (repo, mut db) = project_repo();
  let id = ResourceId::generate();
  rusqlite::Connection::open(repo.path().join(".orchester/orchester.db"))
    .expect("open raw database")
    .execute(
      "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, 'mcp_server', 'bare', 'Bare', 1)",
      [id.to_string()],
    )
    .expect("a resource without version or draft");
  let server: TeamMcpServer =
    serde_json::from_value(serde_json::json!({"name":"bare","launch":{"kind":"program","program":"npx"}}))
      .expect("a team MCP server");

  let refused = db.save_draft(id, &Draft::new(server, None, at(2)));

  assert!(matches!(refused, Err(StoreError::NoFirstVersion(refused)) if refused == id));
  assert!(db.draft::<TeamMcpServer>(id).unwrap().is_none());
}
