//! Archiving what a local project has and restoring it as it was (ORC-842): an archived resource leaves the lists but
//! keeps its versions, a pipeline step still names it, and restoring brings it back; the same for a local MCP server,
//! which has no versions. The team's resources are not archived here.
//! `cargo test -p orchester-store --test local_archive local_archive`.

mod support;

use std::{collections::BTreeSet, fs};

use orchester_core::{
  connection::mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
  id::{ConnectionId, ProjectId, ResourceId},
  project::{Project, ProjectKey},
  resource::{
    AgentTargets, CommandProgram, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceKind, Slug,
  },
  store::{McpServerStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};
use support::{at, name};
use tempfile::TempDir;

fn project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    name("Kontur"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn instruction(db: &mut ProjectDb, slug: &str) -> ResourceId {
  let body = Instruction::new(
    PromptText::new("Be brief.").expect("valid prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("valid instruction");
  let id = db
    .create_resource(Slug::new(slug).expect("slug"), name(slug), &body, at(1))
    .expect("create instruction")
    .id();
  db.publish_draft::<Instruction>(id, at(2)).expect("publish");
  id
}

fn server(db: &mut ProjectDb, label: &str) -> ConnectionId {
  let transport = McpTransport::Stdio(StdioServer::new(
    CommandProgram::new("docs-mcp").expect("valid program"),
    Vec::new(),
    None,
  ));
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(label).expect("valid name"),
    transport,
    McpEnvironment::default(),
    at(3),
  )
  .expect("a valid server");
  db.create_mcp_server(&server).expect("create server");
  server.id()
}

fn listed(db: &ProjectDb, id: ResourceId) -> bool {
  db.resources(ResourceKind::Instruction)
    .expect("instructions")
    .iter()
    .any(|resource| resource.id() == id)
}

#[test]
fn local_archive_takes_a_resource_out_of_the_lists_and_restores_it_as_it_was() {
  let (_repo, mut db) = project();
  let rules = instruction(&mut db, "rules");
  assert!(db.archive_local_resource(rules, at(5)).expect("archive"));
  assert!(!listed(&db, rules), "an archived resource leaves the list");
  assert!(
    db.latest_version::<Instruction>(rules).expect("version").is_some(),
    "its versions stay for the runs that pinned it"
  );
  let archived = db
    .archived_local_resources(ResourceKind::Instruction)
    .expect("archived");
  assert_eq!(archived.len(), 1);
  assert_eq!(archived[0].0.id(), rules);
  assert_eq!(archived[0].1, at(5));
  assert!(
    !db.archive_local_resource(rules, at(6)).expect("again"),
    "archived once"
  );
  assert!(db.restore_local_resource(rules).expect("restore"));
  assert!(listed(&db, rules));
  assert!(
    db.archived_local_resources(ResourceKind::Instruction)
      .expect("archived")
      .is_empty()
  );
  assert!(!db.restore_local_resource(rules).expect("again"), "restored once");
}

#[test]
fn local_archive_of_an_unknown_resource_is_not_found() {
  let (_repo, mut db) = project();
  let unknown = ResourceId::generate();
  assert!(matches!(
    db.archive_local_resource(unknown, at(5)),
    Err(StoreError::ResourceNotFound(found)) if found == unknown
  ));
  assert!(matches!(
    db.restore_local_resource(unknown),
    Err(StoreError::ResourceNotFound(found)) if found == unknown
  ));
}

#[test]
fn local_archive_keeps_an_archived_mcp_server_and_restores_it_whole() {
  let (_repo, mut db) = project();
  let docs = server(&mut db, "docs");
  let other = server(&mut db, "figma");
  assert!(db.archive_mcp_server(docs, at(7)).expect("archive"));
  assert!(!db.archive_mcp_server(docs, at(8)).expect("again"));
  assert_eq!(db.archived_mcp_servers().expect("archived"), vec![(docs, at(7))]);
  assert!(
    db.mcp_server(docs).expect("read").is_some(),
    "the server and its settings stay for what pins it"
  );
  assert!(db.restore_mcp_server(docs).expect("restore"));
  assert!(db.archived_mcp_servers().expect("archived").is_empty());
  assert!(!db.restore_mcp_server(other).expect("not archived"));
  let unknown = ConnectionId::generate();
  assert!(matches!(
    db.archive_mcp_server(unknown, at(9)),
    Err(StoreError::ConnectionNotFound(found)) if found == unknown
  ));
}

#[test]
fn local_archive_goes_with_a_deleted_mcp_server() {
  let (_repo, mut db) = project();
  let docs = server(&mut db, "docs");
  db.archive_mcp_server(docs, at(7)).expect("archive");
  db.delete_unused_mcp_server(docs, &|_| Ok(Vec::new())).expect("delete");
  assert!(db.archived_mcp_servers().expect("archived").is_empty());
}
