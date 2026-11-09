//! The Agents hub of a local project (`Agents · v2` `acFkz`, L1–L14, ORC-842): a filter by kind over a list grouped by
//! what the person has to do, the panel of the chosen entry, a local MCP server that has no versions and is edited in
//! place, and archive and restore. `cargo test -p orchester-app --test agents local_hub`.

use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider,
    mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{
    AgentProfile, CommandProgram, ModelId, PromptText, ProviderSettings, RepositoryAccess, ResourceKind, Slug,
  },
  store::{ConnectionStore, McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::agents::data;

use super::{harness::Agents, support::project_id};

const SAVE: &str = "local-hub-mcp-save";
const CANCEL: &str = "local-hub-mcp-cancel";

/// A local MCP server called `name` that runs the program `docs-mcp`, which no computer has.
fn server(agents: &Agents, name: &str) -> ConnectionId {
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(name).expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("docs-mcp").expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::default(),
    Timestamp::now(),
  )
  .expect("server");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  McpServerStore::create_mcp_server(&mut db, &server).expect("stored");
  server.id()
}

fn open(agents: &mut Agents, view: HubView) {
  agents
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::AgentsHub(view)));
  agents.until("the hub", |s| {
    s.exists("local-hub-filter") && !s.exists("local-hub-loading")
  });
}

fn show(agents: &mut Agents, server: ConnectionId) {
  open(agents, HubView::on_server(HubFilter::McpServers, server));
  agents.until("the server's panel", |s| s.exists(SAVE));
}

#[test]
fn local_hub_counts_each_kind_and_lists_a_server_without_a_version() {
  let mut agents = Agents::new(true);
  agents.profile(true);
  let docs = server(&agents, "docs");
  open(&mut agents, HubView::default());
  let filter = |agents: &mut Agents, name: &str| agents.text(&format!("local-hub-filter-{name}"));
  assert!(
    filter(&mut agents, "all").contains('2'),
    "{}",
    filter(&mut agents, "all")
  );
  assert!(filter(&mut agents, "profiles").contains('1'));
  assert!(filter(&mut agents, "mcp_servers").contains('1'));
  assert!(filter(&mut agents, "archived").contains('0'));
  let row = format!("local-hub-row-mcp-{docs}");
  assert!(agents.exists(&row));
  let line = agents.text(&format!("{row}-line"));
  assert!(!line.contains('v'), "no version on a server's row: {line}");
  // The program `docs-mcp` is not on this computer: the row is under Needs you, and says what to do.
  assert!(agents.exists("local-hub-section-needs"));
  assert!(line.contains("program path"), "{line}");
}

#[test]
fn local_hub_head_offers_share_with_a_team_and_new_of_the_filter() {
  let mut agents = Agents::new(true);
  open(&mut agents, HubView::new(HubFilter::McpServers, None));
  assert_eq!(agents.text("local-hub-share"), "Share with a team…");
  assert_eq!(agents.text("local-hub-new"), "New MCP server");
  open(&mut agents, HubView::new(HubFilter::Archived, None));
  assert!(!agents.exists("local-hub-new"), "nothing is made under Archived");
  assert!(agents.exists("local-hub-share"));
}

#[test]
fn local_hub_head_reaches_memory_and_the_import_of_the_kind_the_filter_holds() {
  let mut agents = Agents::new(true);
  open(&mut agents, HubView::new(HubFilter::Profiles, None));
  assert_eq!(agents.text("local-hub-memory"), "Memory");
  assert_eq!(agents.text("local-hub-import"), "Import");
  open(&mut agents, HubView::new(HubFilter::McpServers, None));
  assert!(agents.exists("local-hub-memory"));
  assert!(
    !agents.exists("local-hub-import"),
    "an MCP server is not imported from the repository"
  );
  open(&mut agents, HubView::new(HubFilter::Profiles, None));
  agents.click("local-hub-import");
  assert_eq!(
    agents.window.navigation().route_now(),
    Route::Project(
      project_id(),
      ProjectPage::Import(orchester_app::routes::ImportTab::Agents)
    )
  );
  open(&mut agents, HubView::new(HubFilter::Profiles, None));
  agents.click("local-hub-memory");
  assert_eq!(
    agents.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Memory(None))
  );
}

#[test]
fn local_hub_server_panel_edits_in_place_with_save_and_cancel_and_no_versions() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  for id in ["local-hub-versions", "local-hub-publish", "local-hub-draft"] {
    assert!(!agents.exists(id), "{id}: a local server has no versions");
  }
  let row = format!("local-hub-row-mcp-{docs}");
  agents.set("connection-mcp-name", "docs-2");
  agents.until("unsaved", |s| {
    s.text(&format!("{row}-line")).contains("Unsaved changes")
  });
  assert!(agents.text("local-hub-chip-0").contains("Unsaved changes"));
  // Cancel puts back what was saved.
  agents.click(CANCEL);
  agents.until("cancelled", |s| {
    !s.text(&format!("{row}-line")).contains("Unsaved changes")
  });
  assert_eq!(agents.value("connection-mcp-name"), "docs");
  // Save applies the change to the project at once.
  agents.set("connection-mcp-name", "docs-2");
  agents.until("saveable", |s| s.exists(SAVE));
  // A new name widens what agents may do (a run's answers by the server's name would apply to it): it is asked first.
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("local-hub-mcp-confirm-save-confirm"));
  agents.press_until("local-hub-mcp-confirm-save-confirm", "saved", |s| {
    let db = ProjectDb::open(&s.root).expect("database");
    McpServerStore::mcp_servers(&db)
      .expect("servers")
      .iter()
      .any(|server| server.name().as_str() == "docs-2")
  });
}

#[test]
fn local_hub_a_save_that_is_refused_says_why_in_place_and_keeps_every_edit() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  let row = format!("local-hub-row-mcp-{docs}");
  // A program found nowhere is not stored: the save is refused, after the person confirmed what it widens.
  agents.set("connection-mcp-name", "docs-2");
  agents.set("connection-mcp-program", "docs-mcp-found-nowhere");
  agents.until("saveable", |s| s.exists(SAVE));
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("local-hub-mcp-confirm-save-confirm"));
  agents.press_until("local-hub-mcp-confirm-save-confirm", "refused", |s| {
    s.exists("local-hub-mcp-refused")
  });
  assert!(agents.text("local-hub-mcp-refused").contains("Couldn't save docs"));
  assert!(agents.text("local-hub-mcp-refused").contains("docs-mcp-found-nowhere"));
  assert_eq!(agents.value("connection-mcp-name"), "docs-2", "what was typed stays");
  assert_eq!(agents.value("connection-mcp-program"), "docs-mcp-found-nowhere");
  agents.until("the row says so", |s| {
    s.text(&format!("{row}-line")).contains("Couldn't save")
  });
  let db = ProjectDb::open(&agents.root).expect("database");
  assert!(
    McpServerStore::mcp_servers(&db)
      .expect("servers")
      .iter()
      .all(|server| server.name().as_str() == "docs"),
    "nothing was saved, not even the other fields"
  );
  // Cancel puts back what was saved.
  agents.click(CANCEL);
  agents.until("cancelled", |s| !s.exists("local-hub-mcp-refused"));
  assert_eq!(agents.value("connection-mcp-name"), "docs");
}

#[test]
fn local_hub_archive_an_mcp_server_then_restore_it_from_the_archived_view() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  agents.click("local-hub-more");
  agents.click("local-hub-archive");
  agents.until("the confirmation", |s| s.exists("team-hub-archive-confirm"));
  assert!(agents.text("team-hub-archive-body").contains("leaves every list"));
  agents.press_until("team-hub-archive-confirm", "archived", |s| {
    !s.exists(&format!("local-hub-row-mcp-{docs}"))
  });
  open(&mut agents, HubView::new(HubFilter::Archived, None));
  let row = format!("local-hub-row-mcp-{docs}");
  agents.until("listed as archived", |s| s.exists(&row));
  assert!(agents.text("local-hub-filter-archived").contains('1'));
  assert!(agents.text(&format!("{row}-line")).contains("Archived"));
  open(&mut agents, HubView::on_server(HubFilter::Archived, docs));
  agents.until("its panel", |s| s.exists("archive-restore"));
  assert_eq!(agents.text("archive-restore"), "Restore docs");
  agents.press_until("archive-restore", "restored", |s| !s.exists(&row));
  open(&mut agents, HubView::new(HubFilter::McpServers, None));
  agents.until("back in the list", |s| s.exists(&format!("local-hub-row-mcp-{docs}")));
}

#[test]
fn local_hub_archive_a_profile_lists_it_as_archived() {
  let mut agents = Agents::new(true);
  let id: ResourceId = agents.profile(true);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(id)));
  agents.until("the panel", |s| s.exists("local-hub-more"));
  agents.click("local-hub-more");
  agents.click("local-hub-archive");
  agents.press_until("team-hub-archive-confirm", "archived", |s| {
    s.listed().profile(id).is_none()
  });
  open(&mut agents, HubView::new(HubFilter::Archived, None));
  agents.until("listed as archived", |s| s.exists(&format!("local-hub-row-{id}")));
  let db = ProjectDb::open(&agents.root).expect("database");
  assert_eq!(
    ResourceStore::resources(&db, ResourceKind::AgentProfile)
      .expect("profiles")
      .len(),
    0
  );
}

/// Builder, a Claude model on a connection the project no longer has, published as v1; and two Claude Code connections.
fn builder_without_connection(agents: &Agents) -> (ResourceId, [ConnectionId; 2]) {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let claude = ["Claude 2", "Home"].map(|label| {
    let connection = AgentConnection::new(
      ConnectionId::generate(),
      Name::new(label).expect("name"),
      AgentProvider::ClaudeCodeCli,
      agents.root.join(format!("{label}.exe")),
      Timestamp::now(),
    )
    .expect("connection");
    db.create_connection(&connection).expect("stored");
    connection.id()
  });
  let profile = AgentProfile::new(
    Name::new("Builder").expect("name"),
    "Builds the change".into(),
    PromptText::new("Implement the approved plan.").expect("instructions"),
    ConnectionId::generate(),
    ModelId::new("claude-opus-5-5").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  ));
  let id = db
    .create_resource(
      Slug::new("builder").expect("slug"),
      Name::new("Builder").expect("name"),
      &profile,
      Timestamp::now(),
    )
    .expect("resource")
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now())
    .expect("published");
  (id, claude)
}

#[test]
fn local_hub_a_profile_whose_connection_is_gone_asks_which_of_its_kind_runs_it() {
  let mut agents = Agents::new(true);
  let (id, [claude_2, _]) = builder_without_connection(&agents);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(id)));
  agents.until("the panel", |s| s.exists("local-hub-fix-use"));
  assert_eq!(
    agents.text(&format!("local-hub-row-{id}-line")),
    "Choose a Claude Code connection"
  );
  assert_eq!(agents.text("local-hub-fix-use"), "Use Claude 2 for Builder");
  for part in [
    "local-hub-profile-pick",
    "local-hub-profile-model",
    "team-hub-edit",
    "local-hub-profile-versions",
  ] {
    assert!(agents.exists(part), "{part} is the panel's own");
  }
  for page in ["agent-role", "agent-crumb", "agents-hub-title"] {
    assert!(!agents.exists(page), "{page} belongs to the editor's page");
  }
  agents.press_until("local-hub-fix-use", "published on Claude 2", |s| {
    s.listed()
      .profile(id)
      .is_some_and(|entry| entry.profile.connection() == claude_2)
  });
  agents.until("the fix is gone", |s| !s.exists("local-hub-fix-use"));
  let line = agents.text(&format!("local-hub-row-{id}-line"));
  assert!(line.contains("Claude 2"), "{line}");
}

#[test]
fn local_hub_a_choice_that_is_refused_says_why_in_place() {
  let mut agents = Agents::new(true);
  let (id, _) = builder_without_connection(&agents);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(id)));
  agents.until("the panel", |s| s.exists("local-hub-fix-use"));
  // Unpublished edits would go out with the choice, so it is refused until they are published or discarded.
  let listed = agents.listed();
  let body = super::harness::changed(&listed.profile(id).expect("profile").profile);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  data::save_draft(&mut db, id, body, Timestamp::now()).expect("a draft");
  agents.click("local-hub-fix-use");
  agents.until("refused", |s| s.exists("local-hub-profile-error"));
  assert!(agents.text("local-hub-profile-error").contains("not published"));
}
