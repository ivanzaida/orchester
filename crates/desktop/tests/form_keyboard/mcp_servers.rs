//! MCP servers: the MCP servers tab of Connections with each server's row and details, the add
//! dialog driven with the keyboard, and the switches that choose a profile's servers on its
//! Permissions tab, with what the connection's CLI enforces for them.

use std::{collections::BTreeMap, path::Path};

use lurq::node::element::ElementRef;
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    mcp::{
      HttpServer, McpEnvValue, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, McpUrl,
      StdioServer, WorkingDirectory,
    },
  },
  id::{ConnectionId, SecretId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{AgentProfile, CommandArg, CommandProgram, ModelId, PromptText, ProviderSettings, Slug, VariableName},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::harness::Desktop;

const INSPECTOR: &str = "connection-inspector";
/// More presses than the screens have stops.
const PRESSES: usize = 40;

pub(crate) fn contains_text(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content().is_some_and(|content| content.contains(text))
    || element.children().into_iter().any(|child| contains_text(child, text))
}

fn shows(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  let element = desktop
    .tree
    .get_element_by_id(id)
    .unwrap_or_else(|| panic!("{id} is shown"));
  contains_text(element, text)
}

fn server(name: &str, transport: McpTransport, environment: McpEnvironment) -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(name).expect("valid name"),
    transport,
    environment,
    Timestamp::now(),
  )
  .expect("a valid server")
}

fn stdio(program: &str, folder: Option<&str>) -> McpTransport {
  McpTransport::Stdio(StdioServer::new(
    CommandProgram::new(program).expect("valid program"),
    vec![
      CommandArg::new("-y").expect("valid"),
      CommandArg::new("@acme/docs-mcp").expect("valid"),
    ],
    folder.map(|folder| WorkingDirectory::new(folder).expect("valid folder")),
  ))
}

/// `docs`, a program with a secret variable; `knowledge`, a remote server that is turned off; and
/// `local`, a program set to start in `tools`. Returns the servers' ids.
fn seed_servers(repo: &Path) -> Vec<ConnectionId> {
  let mut db = ProjectDb::open(repo).expect("project database");
  let token = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("DOCS_TOKEN").expect("valid"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    Timestamp::now(),
  );
  db.create_secret(&token).expect("secret");
  let environment = McpEnvironment::new(BTreeMap::from([(
    VariableName::new("DOCS_TOKEN").expect("valid"),
    McpEnvValue::Secret(token.id()),
  )]))
  .expect("valid variables");
  let docs = server("docs", stdio("npx", None), environment);
  let mut knowledge = server(
    "knowledge",
    McpTransport::Http(HttpServer::new(
      McpUrl::new("https://mcp.example.dev/mcp").expect("valid"),
      Some(token.id()),
    )),
    McpEnvironment::default(),
  );
  knowledge.set_enabled(false);
  let local = server("local", stdio("node", Some("tools")), McpEnvironment::default());
  for server in [&docs, &knowledge, &local] {
    db.create_mcp_server(server).expect("server");
  }
  vec![docs.id(), knowledge.id(), local.id()]
}

fn open_mcp_tab(desktop: &mut Desktop) {
  desktop.wait_for("nav-connections");
  desktop.click("nav-connections");
  desktop.wait_for("connections-tab-mcp");
  desktop.click("connections-tab-mcp");
}

#[test]
fn mcp_tab_lists_the_servers_with_their_details_and_turns_one_off() {
  let mut desktop = Desktop::with_project();
  let ids = seed_servers(desktop.repo());
  open_mcp_tab(&mut desktop);
  desktop.wait_for(INSPECTOR);
  let (docs, knowledge) = (format!("connection-{}", ids[0]), format!("connection-{}", ids[1]));
  assert!(shows(&mut desktop, &docs, "Program · npx"));
  assert!(shows(&mut desktop, &docs, "On"));
  assert!(shows(&mut desktop, &knowledge, "Remote MCP · mcp.example.dev"));
  assert!(shows(&mut desktop, &knowledge, "Turned off"));
  // The first server's details; a secret is named, its value never shown.
  for text in [
    "docs",
    "Program on this computer",
    "-y @acme/docs-mcp",
    "The worktree",
    "From DOCS_TOKEN (staging)",
  ] {
    assert!(shows(&mut desktop, INSPECTOR, text), "{text}");
  }
  assert!(shows(&mut desktop, INSPECTOR, "cannot list their tools yet"));
  desktop.click("mcp-toggle");
  desktop.wait_until("the server is turned off", |desktop| {
    shows(desktop, &docs, "Turned off")
  });
  let db = ProjectDb::open(desktop.repo()).expect("project database");
  assert!(!db.mcp_server(ids[0]).expect("read").expect("stored").enabled());
  // Other tabs do not list MCP servers; All connections does.
  desktop.click("connections-tab-providers");
  assert!(desktop.tree.get_element_by_id(&docs).is_none());
  desktop.click("connections-tab-all");
  desktop.wait_for(&docs);
}

#[test]
fn mcp_dialog_adds_a_server_with_the_keyboard() {
  let mut desktop = Desktop::with_project();
  open_mcp_tab(&mut desktop);
  desktop.wait_for("empty-add-connection");
  assert!(
    desktop
      .tree
      .root()
      .is_some_and(|root| contains_text(root, "No MCP servers yet"))
  );
  desktop.click("add-connection");
  desktop.wait_for("connection-dialog");
  assert!(shows(&mut desktop, "connection-dialog", "Add MCP server"));
  desktop.tab_to("mcp-name", PRESSES);
  desktop.type_text("Docs");
  assert!(shows(&mut desktop, "connection-dialog", "Use small letters"));
  for _ in 0.."Docs".len() {
    desktop.key("Backspace", "Backspace");
  }
  desktop.type_text("docs");
  desktop.tab_to("mcp-program", PRESSES);
  desktop.type_text("npx");
  desktop.tab_to("mcp-arguments", PRESSES);
  desktop.type_text("-y");
  // Enter in a single-line field saves the server once it can be saved.
  desktop.tab_to("mcp-program", PRESSES);
  desktop.enter();
  desktop.wait_until("the dialog closes", |desktop| {
    desktop.tree.get_element_by_id("connection-dialog").is_none()
  });
  let db = ProjectDb::open(desktop.repo()).expect("project database");
  let servers = db.mcp_servers().expect("servers");
  assert_eq!(servers.len(), 1);
  assert_eq!(servers[0].name().as_str(), "docs");
  let McpTransport::Stdio(stdio) = servers[0].transport() else {
    panic!("a program the CLI starts");
  };
  assert_eq!(stdio.program().as_str(), "npx");
  assert_eq!(stdio.args().len(), 1);
  desktop.wait_for(&format!("connection-{}", servers[0].id()));
}

#[test]
fn mcp_dialog_traps_tab_and_names_what_blocks_saving() {
  let mut desktop = Desktop::with_project();
  open_mcp_tab(&mut desktop);
  desktop.wait_for("empty-add-connection");
  desktop.click("add-connection");
  desktop.wait_for("connection-dialog");
  assert!(shows(&mut desktop, "connection-save-reason", "Enter a name"));
  let controls = [
    "connection-dialog-close",
    "mcp-name",
    "mcp-program",
    "mcp-folder",
    "mcp-arguments",
    "mcp-variables",
    "connection-cancel",
    "connection-save",
  ];
  // Selects have no id of their own and show up as unnamed stops.
  for stop in desktop.stops(PRESSES, false).into_iter().flatten() {
    assert!(controls.contains(&stop.as_str()), "Tab left the dialog for {stop}");
  }
}

/// Chooses the option of the select `id` whose label contains `label`.
fn choose(desktop: &mut Desktop, id: &str, label: &str) {
  let mut select = desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("the select {id}"));
  let index = select
    .labels()
    .iter()
    .position(|option| option.contains(label))
    .unwrap_or_else(|| panic!("{id} offers {label}"));
  select.commit(index);
  desktop.pass();
}

#[test]
fn mcp_dialog_says_when_there_is_no_secret_to_add() {
  let mut desktop = Desktop::with_project();
  open_mcp_tab(&mut desktop);
  desktop.wait_for("empty-add-connection");
  desktop.click("add-connection");
  desktop.wait_for("mcp-add-secret");
  // Without secrets the button cannot be pressed and says why; Tab passes over it.
  assert!(shows(&mut desktop, "mcp-add-secret", "This project has no secrets"));
  assert!(
    desktop
      .tree
      .get_element_by_id_mut("mcp-add-secret")
      .and_then(|element| element.as_select())
      .is_none()
  );
  let stops = desktop.stops(PRESSES, false);
  assert!(!stops.contains(&Some("mcp-add-secret".to_owned())));
}

#[test]
fn mcp_dialog_adds_a_variable_from_a_secret() {
  let mut desktop = Desktop::with_project();
  seed_servers(desktop.repo());
  open_mcp_tab(&mut desktop);
  desktop.wait_for(INSPECTOR);
  desktop.click("add-connection");
  desktop.wait_for("mcp-add-secret");
  assert!(shows(&mut desktop, "mcp-add-secret", "Add a variable from a secret…"));
  desktop.tab_to("mcp-variables", PRESSES);
  desktop.type_text("DOCS_INDEX=docs");
  choose(&mut desktop, "mcp-add-secret", "DOCS_TOKEN (staging)");
  assert_eq!(
    desktop.value("mcp-variables"),
    "DOCS_INDEX=docs
DOCS_TOKEN=${secret:DOCS_TOKEN@staging}"
  );
  assert!(shows(&mut desktop, "mcp-add-secret", "Add a variable from a secret…"));
}

#[test]
fn mcp_dialog_says_who_starts_a_program_and_who_reaches_a_remote_server() {
  let mut desktop = Desktop::with_project();
  let ids = seed_servers(desktop.repo());
  open_mcp_tab(&mut desktop);
  desktop.wait_for(INSPECTOR);
  desktop.click("edit-connection");
  desktop.wait_for("connection-dialog");
  assert!(shows(&mut desktop, "connection-dialog", "Started by"));
  assert!(shows(
    &mut desktop,
    "mcp-started-by",
    "The agent’s CLI, with each agent"
  ));
  desktop.click("connection-cancel");
  desktop.click(&format!("connection-{}", ids[1]));
  desktop.wait_until("the remote server's details", |desktop| {
    shows(desktop, INSPECTOR, "Remote MCP")
  });
  desktop.click("edit-connection");
  desktop.wait_for("connection-dialog");
  assert!(shows(&mut desktop, "connection-dialog", "Reached by"));
  assert!(!shows(&mut desktop, "connection-dialog", "Started by"));
  assert!(shows(&mut desktop, "mcp-started-by", "The agent’s CLI, over HTTP"));
}

fn agent_connection(db: &mut ProjectDb, repo: &Path, provider: AgentProvider) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur").expect("valid"),
    provider,
    repo.join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute executable");
  connection.record_check(ConnectionCheck {
    at: Timestamp::now(),
    result: CheckResult::Available {
      version: "2.1.89".into(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(
        CatalogSource::Provider,
        vec![CatalogModel::new(ModelId::new("haiku").expect("model"), Vec::new())],
      ),
    },
  });
  let id = connection.id();
  db.create_connection(&connection.into_connection()).expect("connection");
  id
}

/// A Reviewer on Claude Code that already gets `docs`.
fn seed_profile(repo: &Path, docs: ConnectionId) {
  let mut db = ProjectDb::open(repo).expect("project database");
  let claude = agent_connection(&mut db, repo, AgentProvider::ClaudeCodeCli);
  let body = AgentProfile::new(
    Name::new("Reviewer").expect("valid"),
    String::new(),
    PromptText::new("Review.").expect("prompt"),
    claude,
    ModelId::new("haiku").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::On,
  ))
  .with_mcp_servers(McpServerSelection::new(vec![docs]).expect("one server"));
  db.create_resource(
    Slug::new("reviewer").expect("slug"),
    Name::new("Reviewer").expect("valid"),
    &body,
    Timestamp::now(),
  )
  .expect("profile");
}

#[test]
fn agent_profile_chooses_its_mcp_servers_on_the_permissions_tab() {
  let mut desktop = Desktop::with_project();
  let ids = seed_servers(desktop.repo());
  seed_profile(desktop.repo(), ids[0]);
  desktop.wait_for("nav-agents");
  desktop.click("nav-agents");
  desktop.wait_for("agent-role");
  desktop.press("agent-tab-permissions");
  desktop.wait_for("agent-mcp");
  // Each server has a switch; why an agent would work without a server is said under it.
  assert!(shows(&mut desktop, "agent-mcp", "Remote MCP · mcp.example.dev"));
  assert!(shows(&mut desktop, "agent-mcp", "Turned off in Connections"));
  assert!(shows(&mut desktop, "agent-mcp", "Claude Code can’t start it in tools"));
  assert!(shows(
    &mut desktop,
    "agent-enforcement",
    "Only the chosen MCP server loads"
  ));
  assert!(shows(&mut desktop, "agent-enforcement", "--allowedTools mcp__*"));
  assert!(shows(
    &mut desktop,
    "agent-enforcement",
    "Servers work outside these limits"
  ));
  // The switches are form controls. Claude Code cannot start `local` in its folder and `knowledge`
  // is turned off, so choosing them loads nothing more.
  desktop.tab_to("agent-mcp-local", PRESSES);
  desktop.enter();
  desktop.tab_to("agent-mcp-knowledge", PRESSES);
  desktop.enter();
  assert!(shows(
    &mut desktop,
    "agent-enforcement",
    "Only the chosen MCP server loads"
  ));
  assert!(!shows(&mut desktop, "agent-enforcement", "Only the 3 chosen"));
  desktop.press("agent-mcp-knowledge");
  desktop.press("agent-mcp-docs");
  desktop.press("agent-save-draft");
  desktop.wait_until("the draft is saved", |desktop| {
    let db = ProjectDb::open(desktop.repo()).expect("project database");
    let profile = db
      .resources(orchester_core::resource::ResourceKind::AgentProfile)
      .expect("profiles")[0]
      .id();
    db.draft::<AgentProfile>(profile)
      .expect("read")
      .is_some_and(|draft| draft.body().mcp_servers().ids() == [ids[2]])
  });
}
