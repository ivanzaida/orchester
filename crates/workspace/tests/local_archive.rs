//! Restoring what a local project archived brings it back as it was; what an archive or restore changes names the
//! screens that read it again (ORC-842). Archiving itself cancels runs through a runner and is checked where it is
//! asked (`pinning_mcp_server`, `used_by_mcp_server`, the store's marks).
//!
//! `cargo test -p orchester-workspace --test local_archive local_archive`

mod support;

use orchester_core::{
  connection::mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
  id::ConnectionId,
  resource::{AgentProfile, CommandProgram, ResourceKind},
  store::{McpServerStore, ResourceStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  local_archive::{Archivable, restore},
  tasks::outside::Outside,
};
use support::{Fixture, at};

#[test]
fn local_archive_restore_brings_back_a_resource_and_a_server_as_they_were() {
  let mut fixture = Fixture::new();
  let builder = fixture.db.resources(ResourceKind::AgentProfile).expect("profiles")[0].id();
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("figma").expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("figma-mcp").expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::default(),
    at(1),
  )
  .expect("server");
  fixture.db.create_mcp_server(&server).expect("stored");
  fixture.db.archive_local_resource(builder, at(20)).expect("archived");
  fixture.db.archive_mcp_server(server.id(), at(21)).expect("archived");
  let runtime = tokio::runtime::Runtime::new().expect("runtime");
  let root = fixture.repo.path();

  runtime
    .block_on(restore(root, Archivable::Resource(builder)))
    .expect("restored");
  runtime
    .block_on(restore(root, Archivable::McpServer(server.id())))
    .expect("restored");

  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.resources(ResourceKind::AgentProfile)
      .expect("profiles")
      .iter()
      .any(|found| found.id() == builder),
    "the profile is back in the lists"
  );
  assert!(db.latest_version::<AgentProfile>(builder).expect("version").is_some());
  assert!(db.archived_mcp_servers().expect("archived").is_empty());
  assert_eq!(db.mcp_server(server.id()).expect("read"), Some(server), "as it was");
}

#[test]
fn local_archive_changes_name_what_each_screen_reads_again() {
  let server = Archivable::McpServer(ConnectionId::generate());
  assert!(server.changes(None).contains(&Outside::Connections));
  assert!(server.changes(None).contains(&Outside::Profiles));
  let profile = Archivable::Resource(orchester_core::id::ResourceId::generate());
  assert_eq!(
    profile.changes(Some(ResourceKind::AgentProfile)),
    [Outside::Profiles, Outside::Pipelines]
  );
  assert_eq!(profile.changes(Some(ResourceKind::Skill)), [Outside::Skills]);
}
