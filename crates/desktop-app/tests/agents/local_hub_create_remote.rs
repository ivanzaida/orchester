//! The New MCP server form of a local project (`Agents · v2` L4, ORC-843) beyond a program with a secret: a remote
//! server with each authorization and the value of the secret it sends, the arguments and the tools that run without
//! asking, and a value typed for a variable that is gone again.
//! `cargo test -p orchester-app --test agents local_hub_create_remote`.

use std::path::Path;

use orchester_app::routes::{HubFilter, ProjectPage, Route};
use orchester_core::{
  connection::mcp::{BasicUsername, McpAuthorization, McpHeaderName, McpServer, McpTransport},
  secret::Secret,
  store::McpServerStore,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  local_hub_create::{TYPED, choose, figma_token, project_secret, servers, show_form},
  support::project_id,
};

const REMOTE: &str = "A remote server (HTTP)";

/// What a secret is called in a select: its name and environment.
fn label(secret: &Secret) -> String {
  format!("{} ({})", secret.name(), secret.environment().as_str())
}

fn stored(agents: &Agents, name: &str) -> McpServer {
  ProjectDb::open(&agents.root)
    .expect("database")
    .mcp_servers()
    .expect("servers")
    .into_iter()
    .find(|server| server.name().as_str() == name)
    .unwrap_or_else(|| panic!("the server {name} is stored"))
}

fn authorization(agents: &Agents, name: &str) -> McpAuthorization {
  let McpTransport::Http(http) = stored(agents, name).transport().clone() else {
    panic!("a remote server");
  };
  http.authorization().clone()
}

/// Waits until Create made the server and the page shows its panel.
fn until_made(agents: &mut Agents) {
  agents.until("the new server's panel", |s| {
    matches!(
      s.window.navigation().route_now(),
      Route::Project(_, ProjectPage::AgentsHub(view)) if view.server.is_some() && !view.adding
    )
  });
}

/// The form of a remote server `name` with the scheme `scheme` (its option's words) and `extra` typed into the field
/// that scheme asks for.
fn remote_form(agents: &mut Agents, name: &str, scheme: &str, extra: Option<(&str, &str)>) {
  show_form(agents, HubFilter::McpServers);
  agents.set("team-new-mcp-name", name);
  choose(agents, "local-hub-mcp-runs-as", REMOTE);
  agents.until("the endpoint", |s| s.exists("connection-mcp-url"));
  agents.set("connection-mcp-url", "https://mcp.example.dev/mcp");
  choose(agents, "connection-mcp-authorization", scheme);
  if let Some((id, value)) = extra {
    agents.set(id, value);
  }
}

/// A remote server's endpoint and authorization are rows of the panel like the program's: one label each, with the
/// field across the row.
#[test]
fn local_hub_create_a_remote_server_lays_out_one_label_a_row() {
  let mut agents = Agents::new(true);
  project_secret(&agents, "DOCS_TOKEN");
  remote_form(&mut agents, "docs", "Bearer token", None);
  assert_eq!(agents.text("connection-mcp-url-row-label"), "Endpoint");
  assert_eq!(agents.text("connection-mcp-authorization-row-label"), "Authentication");
  assert!(
    !agents.exists("connection-mcp-url-label") && !agents.exists("connection-mcp-authorization-label"),
    "no second label inside the row's value"
  );
}

/// Whether the secret store holds a value for `secret`.
pub(super) fn kept(agents: &Agents, secret: &Secret) -> Option<String> {
  let services = agents.window.launch.services().expect("services");
  let read = services.vault.get().read(&secret.vault_key(project_id()));
  read.ok().map(|value| value.expose().to_owned())
}

/// Types `TYPED` as the value of `secret`, in the row it has once the form takes it.
fn type_value(agents: &mut Agents, secret: &Secret) {
  let row = format!("local-hub-secret-{}-replace", secret.name());
  agents.until("the secret's row", |s| s.exists(&row));
  agents.click(&row);
  agents.until("its dialog", |s| s.exists("local-hub-secret-value"));
  agents.set("local-hub-secret-value", TYPED);
  agents.click("local-hub-secret-keep");
}

#[test]
fn local_hub_create_a_remote_mcp_server_is_made_with_each_authorization() {
  let mut agents = Agents::new(true);
  let token = project_secret(&agents, "DOCS_TOKEN");
  let schemes = [
    (
      "basic-docs",
      "Basic: user name and password",
      Some(("connection-mcp-username", "deploy")),
    ),
    ("pasted-docs", "Basic: pasted credential", None),
    (
      "keyed-docs",
      "Custom header",
      Some(("connection-mcp-header", "X-API-Key")),
    ),
    ("bearer-docs", "Bearer token", None),
  ];
  for (name, scheme, extra) in schemes {
    remote_form(&mut agents, name, scheme, extra);
    agents.click("local-hub-new-create");
    agents.until("Create says the secret is missing", |s| {
      s.exists("connection-mcp-secret-error")
    });
    choose(&mut agents, "connection-mcp-secret", &label(&token));
    assert!(!agents.exists("connection-mcp-secret-error"), "{scheme}");
    agents.click("local-hub-new-create");
    until_made(&mut agents);
  }
  let id = token.id();
  assert_eq!(
    authorization(&agents, "basic-docs"),
    McpAuthorization::Basic {
      username: BasicUsername::new("deploy").expect("user"),
      password: id,
    }
  );
  assert_eq!(
    authorization(&agents, "pasted-docs"),
    McpAuthorization::BasicCredential { credential: id }
  );
  assert_eq!(
    authorization(&agents, "keyed-docs"),
    McpAuthorization::Header {
      name: McpHeaderName::new("X-API-Key").expect("header"),
      value: id,
    }
  );
  assert_eq!(
    authorization(&agents, "bearer-docs"),
    McpAuthorization::Bearer { token: id }
  );
  assert_eq!(servers(&agents), 4);
}

#[test]
fn local_hub_create_the_secret_a_remote_server_sends_takes_its_value_with_the_server() {
  let mut agents = Agents::new(true);
  let token = project_secret(&agents, "DOCS_TOKEN");
  remote_form(&mut agents, "bearer-docs", "Bearer token", None);
  choose(&mut agents, "connection-mcp-secret", &label(&token));
  type_value(&mut agents, &token);
  assert!(kept(&agents, &token).is_none(), "nothing is stored before Create");
  agents.click("local-hub-new-create");
  until_made(&mut agents);
  assert_eq!(
    authorization(&agents, "bearer-docs"),
    McpAuthorization::Bearer { token: token.id() }
  );
  assert_eq!(kept(&agents, &token).as_deref(), Some(TYPED));
}

#[test]
fn local_hub_create_authorization_fields_say_why_they_cannot_be_used() {
  let mut agents = Agents::new(true);
  project_secret(&agents, "DOCS_TOKEN");
  remote_form(
    &mut agents,
    "docs",
    "Custom header",
    Some(("connection-mcp-header", "Content-Type")),
  );
  assert!(
    !agents.exists("connection-mcp-secret-error"),
    "nothing is said about the secret before Create"
  );
  agents.click("local-hub-new-create");
  agents.until("the header says why", |s| {
    s.window
      .texts("connection-mcp-header-note")
      .is_some_and(|said| said.join(" ").contains("The agent's CLI sets Content-Type itself"))
  });
  choose(
    &mut agents,
    "connection-mcp-authorization",
    "Basic: user name and password",
  );
  agents.set("connection-mcp-username", "a:b");
  agents.click("local-hub-new-create");
  agents.until("the user name says why", |s| {
    s.window
      .texts("connection-mcp-username-note")
      .is_some_and(|said| said.join(" ").contains("cannot contain a colon"))
  });
  assert!(!agents.exists("connection-mcp-header"), "Basic asks for no header");
  assert_eq!(servers(&agents), 0);
}

#[test]
fn local_hub_create_an_mcp_server_keeps_its_arguments_and_the_tools_that_run_without_asking() {
  let mut agents = Agents::new(true);
  show_form(&mut agents, HubFilter::McpServers);
  agents.set("team-new-mcp-name", "Pencil Design");
  agents.until("the name says why", |s| {
    s.text("team-new-mcp-name-row-sub").contains("small letters, digits")
  });
  agents.set("team-new-mcp-name", "pencil-design");
  // A program outside the project's repository: this test's own.
  let program = std::env::current_exe().expect("the test program");
  agents.set("connection-mcp-program", &program.to_string_lossy());
  agents.set("connection-mcp-arguments", "--app desktop --agent codexCLI");
  agents.set("connection-mcp-tools", "get_editor_state, get_guidelines, batch_get");
  agents.click("local-hub-new-create");
  until_made(&mut agents);
  let server = stored(&agents, "pencil-design");
  let McpTransport::Stdio(stdio) = server.transport() else {
    panic!("a program on this computer");
  };
  assert_eq!(
    Path::new(stdio.program().as_str()),
    program.as_path(),
    "the program is the file itself"
  );
  let args: Vec<&str> = stdio.args().iter().map(|arg| arg.as_str()).collect();
  assert_eq!(args, ["--app", "desktop", "--agent", "codexCLI"]);
  assert_eq!(server.exempt_tools().len(), 3);
  assert!(server.enabled());
}

/// A value typed for a variable that was removed (or left behind by switching to a remote server) is not the server's:
/// Create makes the server, and the secret store keeps nothing.
#[test]
fn local_hub_create_a_value_typed_for_a_variable_that_is_gone_is_not_stored() {
  let mut agents = Agents::new(true);
  let token = figma_token(&agents);
  show_form(&mut agents, HubFilter::McpServers);
  agents.set("team-new-mcp-name", "figma");
  agents.set(
    "connection-mcp-program",
    &std::env::current_exe().expect("a program").to_string_lossy(),
  );
  agents.set(
    "connection-mcp-variables",
    "FIGMA_API_KEY=${secret:FIGMA_TOKEN@development}",
  );
  type_value(&mut agents, &token);
  agents.set("connection-mcp-variables", "");
  agents.until("the secret's row is gone", |s| {
    !s.exists("local-hub-secret-FIGMA_TOKEN-replace")
  });
  agents.click("local-hub-new-create");
  until_made(&mut agents);
  assert_eq!(servers(&agents), 1, "the server is made");
  assert!(
    kept(&agents, &token).is_none(),
    "the value of a variable that is gone is not stored"
  );
}

#[test]
fn local_hub_create_a_value_typed_before_switching_to_a_remote_server_is_not_stored() {
  let mut agents = Agents::new(true);
  let token = figma_token(&agents);
  show_form(&mut agents, HubFilter::McpServers);
  agents.set("team-new-mcp-name", "figma");
  agents.set(
    "connection-mcp-program",
    &std::env::current_exe().expect("a program").to_string_lossy(),
  );
  agents.set(
    "connection-mcp-variables",
    "FIGMA_API_KEY=${secret:FIGMA_TOKEN@development}",
  );
  type_value(&mut agents, &token);
  choose(&mut agents, "local-hub-mcp-runs-as", REMOTE);
  agents.until("the endpoint", |s| s.exists("connection-mcp-url"));
  agents.set("connection-mcp-url", "https://mcp.example.dev/mcp");
  agents.click("local-hub-new-create");
  until_made(&mut agents);
  assert!(matches!(stored(&agents, "figma").transport(), McpTransport::Http(_)));
  assert!(
    kept(&agents, &token).is_none(),
    "the value of a variable that is gone is not stored"
  );
}
