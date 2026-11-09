//! The secret values a saved local MCP server's panel asks for (`Agents · v2` L8, L12, ORC-843): a remote server's
//! authorization secret has its row as a program's variables do, and a value typed for a variable that is gone again
//! is not stored. `cargo test -p orchester-app --test agents local_hub_edit_secrets`.

use orchester_core::{
  connection::mcp::{HttpServer, McpAuthorization, McpEnvironment, McpServer, McpServerName, McpTransport, McpUrl},
  id::ConnectionId,
  secret::Secret,
  store::{McpServerStore, SecretStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  local_hub::{SAVE, TYPED, figma, show, type_secret},
  local_hub_create::project_secret,
  local_hub_create_remote::kept,
};

/// A remote server `docs` that sends the bearer token `token`.
fn remote(agents: &Agents, token: &Secret) -> ConnectionId {
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("docs").expect("name"),
    McpTransport::Http(HttpServer::new(
      McpUrl::new("https://mcp.example.dev/mcp").expect("url"),
      McpAuthorization::Bearer { token: token.id() },
    )),
    McpEnvironment::default(),
    Timestamp::now(),
  )
  .expect("server");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  McpServerStore::create_mcp_server(&mut db, &server).expect("stored");
  server.id()
}

#[test]
fn local_hub_edit_a_saved_remote_servers_authorization_secret_has_its_row_and_takes_a_value() {
  let mut agents = Agents::new(true);
  let token = project_secret(&agents, "DOCS_TOKEN");
  let docs = remote(&agents, &token);
  show(&mut agents, docs);

  agents.until("the secret's row", |s| s.exists("local-hub-secret-DOCS_TOKEN-replace"));
  assert!(
    agents.text("local-hub-secret-DOCS_TOKEN-sub").contains("no value"),
    "nothing is kept for it yet"
  );
  agents.click("local-hub-secret-DOCS_TOKEN-replace");
  agents.until("its dialog", |s| s.exists("local-hub-secret-value"));
  agents.set("local-hub-secret-value", TYPED);
  agents.click("local-hub-secret-keep");
  agents.until("the value waits for Save", |s| {
    s.text("local-hub-secret-DOCS_TOKEN-sub").contains("not saved yet")
  });
  assert!(
    agents.exists("local-hub-secret-DOCS_TOKEN-replace"),
    "the row stays while the fields are valid"
  );

  agents.click(SAVE);
  agents.until("the value is kept", |s| {
    s.text("local-hub-secret-DOCS_TOKEN-sub").contains("never shown")
  });
  assert_eq!(kept(&agents, &token).as_deref(), Some(TYPED));
}

#[test]
fn local_hub_edit_a_value_typed_for_a_variable_that_is_gone_is_not_stored() {
  let mut agents = Agents::new(true);
  let figma = figma(&agents);
  show(&mut agents, figma);
  type_secret(&mut agents);

  agents.set("connection-mcp-variables", "");
  agents.until("the secret's row is gone", |s| {
    !s.exists("local-hub-secret-FIGMA_TOKEN-replace")
  });
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("local-hub-mcp-confirm-save-confirm"));
  agents.click("local-hub-mcp-confirm-save-confirm");
  agents.until("the variable is gone from the saved server", |s| {
    let db = ProjectDb::open(&s.root).expect("database");
    McpServerStore::mcp_servers(&db)
      .expect("servers")
      .iter()
      .any(|server| server.secrets().is_empty())
  });
  let db = ProjectDb::open(&agents.root).expect("database");
  let token = db.secrets().expect("secrets").into_iter().next().expect("FIGMA_TOKEN");
  assert!(
    kept(&agents, &token).is_none(),
    "nothing is stored for the dropped secret"
  );
  assert!(!agents.exists("local-hub-mcp-refused"));
}
