//! With window control on, an MCP client can fill the MCP server form and press its Save or an inspector's Turn
//! on, but a save or Turn on that widens what agents may do asks a person in place, and while that question shows
//! window input is refused as `person_only`: the server is not saved and stays off. A person's press is taken.

use std::path::Path;

use orchester_core::{connection::mcp::McpServer, id::ProjectId, store::McpServerStore, time::Timestamp};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{
  Served,
  kontur::{self, SeedRequest, scenarios},
};

/// Kontur with its connections seeded, served with window control on.
pub(super) fn controlled() -> (Served, ProjectId, std::path::PathBuf) {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let request = SeedRequest {
    data_dir: folder.path().join("bin"),
    repo: folder.path().join("repo-connections"),
    scenario: scenarios::named("connections").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = kontur::seed(&request).expect("the scenario seeds");
  let runner = folder.path().join("no-runner").join("orchester-runner");
  let served = Served::start_with(folder, &seeded.app_database, runner, true);
  (served, seeded.project, seeded.repo)
}

fn server(repo: &Path, name: &str) -> Option<McpServer> {
  ProjectDb::open(repo)
    .expect("the database opens")
    .mcp_servers()
    .expect("servers read")
    .into_iter()
    .find(|server| server.name().as_str() == name)
}

impl Served {
  /// The ref lurq gives element `id`.
  fn reference(&mut self, id: &str) -> String {
    let found = self.ok("lurq_find_by_id", &json!({ "id": id }));
    found
      .as_str()
      .and_then(|line| line.split_whitespace().next())
      .unwrap_or_else(|| panic!("a ref for {id}: {found}"))
      .to_owned()
  }

  /// A client's click on element `id`, scrolled into view first as a client does: a page's actions end its column.
  /// While a question keeps a client's input out, the scroll is refused like the click.
  pub(super) fn click_by_client(&mut self, id: &str) -> Result<Value, Value> {
    let reference = self.reference(id);
    if self
      .call("lurq_interact", &json!({ "action": "scroll_to", "ref": reference }))
      .is_ok()
    {
      self.pass();
    }
    let reference = self.reference(id);
    self.call("lurq_interact", &json!({ "action": "click", "ref": reference }))
  }

  pub(super) fn set_by_client(&mut self, id: &str, value: &str) {
    let reference = self.reference(id);
    self.ok("lurq_set_value", &json!({ "ref": reference, "value": value }));
  }

  /// A person's press on element `id`, in the window itself.
  pub(super) fn click_by_person(&mut self, id: &str) {
    self
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .click();
    self.pass();
  }

  pub(super) fn shows(&mut self, id: &str) -> bool {
    self.tree.get_element_by_id(id).is_some()
  }
}

#[test]
fn domain_window_input_cannot_save_a_widening_mcp_server() {
  let (mut served, project, repo) = controlled();
  served.ok(
    "navigate",
    &json!({ "route": "new_connection", "project": project.to_string(), "kind": "mcp_server" }),
  );
  served.until("the form", |served| served.shows("connection-mcp-name"));
  let program = std::env::current_exe().expect("the test program");
  served.set_by_client("connection-mcp-name", "pencil-design");
  served.set_by_client("connection-mcp-program", &program.to_string_lossy());
  // Save itself takes a client's press: it only asks.
  served.click_by_client("connection-save").expect("Save asks");
  served.until("the question", |served| served.shows("connection-confirm-save-confirm"));
  let refused = served
    .click_by_client("connection-confirm-save-confirm")
    .expect_err("a client cannot confirm");
  assert_eq!(refused["code"], "person_only", "{refused}");
  assert!(
    refused["message"]
      .as_str()
      .is_some_and(|message| message.contains("Connections page")),
    "the client reads which question keeps it out: {refused}"
  );
  for _ in 0..5 {
    served.pass();
  }
  assert!(server(&repo, "pencil-design").is_none(), "nothing is saved");
  served.click_by_person("connection-confirm-save-confirm");
  served.until("the saved server", |_| server(&repo, "pencil-design").is_some());
}

#[test]
fn domain_window_input_cannot_turn_on_an_mcp_server() {
  let (mut served, project, repo) = controlled();
  let postgres = server(&repo, "postgres-dev").expect("postgres-dev");
  assert!(!postgres.enabled());
  served.ok(
    "navigate",
    &json!({
      "route": "connections", "project": project.to_string(), "selected": postgres.id().to_string(),
    }),
  );
  served.until("the server's page", |served| served.shows("connection-switch"));
  served.click_by_client("connection-switch").expect("Turn on asks");
  served.until("the question", |served| {
    served.shows("connection-confirm-turn-on-confirm")
  });
  let refused = served
    .click_by_client("connection-confirm-turn-on-confirm")
    .expect_err("a client cannot confirm");
  assert_eq!(refused["code"], "person_only", "{refused}");
  for _ in 0..5 {
    served.pass();
  }
  assert!(
    !server(&repo, "postgres-dev").expect("postgres-dev").enabled(),
    "it stays off"
  );
  served.click_by_person("connection-confirm-turn-on-confirm");
  served.until("the server on", |_| {
    server(&repo, "postgres-dev").is_some_and(|server| server.enabled())
  });
}

/// The open Connections page reads the servers again when a domain tool changes one.
#[test]
fn domain_tools_changes_show_on_the_open_connections_page() {
  let (mut served, project, repo) = controlled();
  let docs = server(&repo, "kontur-docs").expect("kontur-docs");
  assert!(docs.enabled());
  served.ok(
    "navigate",
    &json!({
      "route": "connections", "project": project.to_string(), "selected": docs.id().to_string(),
    }),
  );
  served.until("the server's page", |served| {
    super::tools_window::text(served, "connection-inspector-status").contains("On")
  });
  served.ok(
    "set_mcp_server_enabled",
    &json!({ "project": project.to_string(), "server": "kontur-docs", "enabled": false }),
  );
  served.until("the server shown off", |served| {
    super::tools_window::text(served, "connection-inspector-status").contains("Turned off")
  });
}
