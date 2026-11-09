//! Restoring what a local project archived brings it back as it was; what an archive or restore changes names the
//! screens that read it again (ORC-842). Archiving itself cancels runs through a runner and is checked where it is
//! asked (`pinning_mcp_server`, `used_by_mcp_server`, the store's marks).
//!
//! `cargo test -p orchester-workspace --test local_archive local_archive`

mod support;

use orchester_core::{
  connection::mcp::{McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  id::ConnectionId,
  name::Name,
  resource::{AgentProfile, CommandProgram, ModelId, PromptText, ProviderSettings, ResourceKind},
  store::{McpServerStore, ResourceStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  agents::{data::ProjectAgents, servers},
  connections::data::ProjectConnections,
  local_archive::{Archivable, Archived, restore},
  pipelines::data::Assignables,
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

fn archived_figma(fixture: &mut Fixture) -> McpServer {
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
  fixture.db.archive_mcp_server(server.id(), at(21)).expect("archived");
  server
}

#[test]
fn local_archive_hides_an_archived_server_from_every_list_a_picker_reads() {
  let mut fixture = Fixture::new();
  let server = archived_figma(&mut fixture);
  let root = fixture.repo.path();

  assert!(
    ProjectAgents::load_in(root).expect("agents").mcp.is_empty(),
    "the profile editor's section"
  );
  assert!(
    ProjectConnections::load_in(root).expect("connections").mcp.is_empty(),
    "the connections read"
  );
  assert!(
    Assignables::load(&ProjectDb::open(root).expect("open"))
      .expect("pipelines")
      .mcp_servers
      .is_empty(),
    "the pipelines' picker"
  );
  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.mcp_servers().expect("read").is_empty(),
    "the tools and the runs read it too"
  );
  assert!(
    db.mcp_server(server.id()).expect("read").is_some(),
    "its panel and Restore still find it"
  );
}

#[test]
fn local_archive_refuses_choosing_an_archived_server_but_keeps_a_choice_that_was_made() {
  let mut fixture = Fixture::new();
  let server = archived_figma(&mut fixture);
  let chooser = AgentProfile::new(
    Name::new("Builder").expect("name"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(vec![server.id()]).expect("selection"));
  let none = AgentProfile::new(
    Name::new("Builder").expect("name"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let offered = ProjectAgents::load_in(fixture.repo.path()).expect("agents").mcp;

  let refused = servers::check(&offered, Some(&none), &chooser).expect_err("a new choice of an archived server");
  assert_eq!(refused.key, "appMcp.errors.mcpServerMissing");
  assert_eq!(
    servers::check(&offered, Some(&chooser), &chooser),
    Ok(()),
    "a profile that already chose it can still be saved: nothing in a picker takes it out"
  );
}

#[test]
fn local_archive_says_when_a_run_could_not_be_stopped() {
  assert_eq!(
    Archived {
      stopped: 2,
      left_running: 0
    }
    .left_running_message(),
    None
  );
  let said = Archived {
    stopped: 1,
    left_running: 2,
  }
  .left_running_message()
  .expect("said");
  assert_eq!(said.key, "localArchive.runsLeftRunning");
  assert_eq!(said.args, [("count", "2".to_owned())]);
}
