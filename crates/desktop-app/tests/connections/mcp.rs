//! MCP servers as a person sets them up in the window: adding one through Add MCP server, changing its tools
//! that run without asking through Edit MCP server, turning it off (asked in place when profiles choose it)
//! and on, and removing one no profile chooses; one a profile chooses cannot be removed. A field that cannot
//! be used says why, and Save says the first reason.

use std::path::Path;

use orchester_app::routes::{NewConnection, ProjectPage};
use orchester_core::connection::mcp::McpTransport;

use super::harness::Seeded;

const TOOLS: &str = "get_editor_state\nget_guidelines\nbatch_get\nsnapshot_layout\nget_screenshot\nget_variables\nbatch_design\nset_variables\nexport_nodes";

#[test]
fn connections_mcp_server_is_added_with_its_tools_that_run_without_asking() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::McpServer);
  seeded.set("connection-mcp-name", "Pencil Design");
  assert!(
    seeded
      .text("connection-form-actions-reason")
      .contains("use small letters, digits"),
    "the first reason is the name's"
  );
  seeded.set("connection-mcp-name", "pencil-design");
  // A program outside the project's repository: this test's own.
  let program = std::env::current_exe().expect("the test program");
  seeded.set("connection-mcp-program", &program.to_string_lossy());
  seeded.set("connection-mcp-arguments", "--app\ndesktop\n--agent\ncodexCLI");
  // Several tool names at once, then each as a pill under the field.
  seeded.set("connection-mcp-exempt-tools", &TOOLS.replace('\n', " "));
  seeded.click("connection-mcp-exempt-tools-add");
  assert_eq!(seeded.text("connection-mcp-tool-8-label"), "export_nodes");
  assert!(
    !seeded.exists("connection-form-actions-reason"),
    "nothing keeps it from being saved"
  );
  // A new server widens what agents may do: a person confirms it in place.
  seeded.click("connection-save");
  seeded.until("the question", |seeded| {
    seeded.exists("connection-confirm-save-confirm")
  });
  assert!(seeded.text("connection-confirm-save").contains("pencil-design"));
  seeded.click("connection-confirm-save-confirm");
  seeded.until("the saved server", |seeded| {
    if let Some(reason) = seeded.window.texts("connection-form-actions-reason") {
      panic!("not saved: {reason:?}");
    }
    seeded.exists("connection-inspector")
  });
  let server = seeded.server("pencil-design").expect("the server is stored");
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
  assert_eq!(server.exempt_tools().len(), 9);
  assert!(server.enabled());
  assert_eq!(seeded.selected(), Some(server.id()));
  assert!(seeded.text("connection-inspector").contains("export_nodes"));
}

#[test]
fn connections_mcp_server_edits_keep_what_is_not_changed() {
  let mut seeded = Seeded::new("connections");
  let docs = seeded.server("kontur-docs").expect("kontur-docs");
  seeded.go(ProjectPage::EditConnection(docs.id()));
  seeded.until("the form", |seeded| seeded.exists("connection-mcp-exempt-tools"));
  assert_eq!(seeded.text("connection-form-actions-reason"), "Nothing changed yet.");
  // Its tools are pills: removing get_page keeps search_docs.
  assert_eq!(seeded.text("connection-mcp-tool-1-label"), "get_page");
  seeded.click("connection-mcp-tool-1-remove");
  assert!(
    !seeded.exists("connection-form-actions-reason"),
    "a change can be saved"
  );
  seeded.click("connection-save");
  seeded.until("the saved server", |seeded| seeded.exists("connection-inspector"));
  let edited = seeded.server("kontur-docs").expect("kontur-docs");
  assert_eq!(edited.exempt_tools().len(), 1);
  assert_eq!(edited.transport(), docs.transport(), "the program stays");
  assert_eq!(edited.environment(), docs.environment(), "the variables stay");
}

/// A server's switch on its page: off asked in place when a profile chooses it, on asked in place as it widens.
#[test]
fn connections_mcp_server_is_turned_off_and_on_in_place() {
  let mut seeded = Seeded::new("connections");
  let docs = seeded.server("kontur-docs").expect("kontur-docs");
  seeded.open(docs.id());
  // Designer chooses it: turning it off asks first.
  seeded.click("connection-switch");
  assert!(seeded.text("connection-confirm").contains("Turn off kontur-docs?"));
  seeded.click("connection-confirm-keep");
  assert!(!seeded.exists("connection-confirm"));
  assert!(seeded.server("kontur-docs").expect("kontur-docs").enabled());
  seeded.click("connection-switch");
  seeded.click("connection-confirm-turn-off");
  seeded.until("the server turned off", |seeded| {
    seeded.server("kontur-docs").is_some_and(|server| !server.enabled())
      && seeded.text("connection-inspector-status").contains("Turned off")
  });
  seeded.click("connection-switch");
  seeded.until("the question", |seeded| {
    seeded.exists("connection-confirm-turn-on-confirm")
  });
  seeded.click("connection-confirm-turn-on-confirm");
  seeded.until("the server turned on", |seeded| {
    seeded.server("kontur-docs").is_some_and(|server| server.enabled())
  });
}

/// Remove, from a server's … menu, asks in place right after *Sends* (`13` `Ptkxo`); Keep it keeps the server, Remove
/// removes it and goes back to Connections. A server a profile chooses cannot be removed, and says which.
#[test]
fn connections_mcp_server_is_removed_in_place() {
  let mut seeded = Seeded::new("connections");
  let docs = seeded.server("kontur-docs").expect("kontur-docs");
  seeded.open(docs.id());
  seeded.click("connection-more");
  assert!(
    seeded.text("connection-more-menu").contains("Designer"),
    "Remove says which profile keeps it"
  );
  seeded.click("connection-more");
  let sentry = seeded.server("sentry").expect("sentry");
  seeded.open(sentry.id());
  seeded.click("connection-more");
  seeded.click("connection-remove");
  assert!(seeded.text("connection-confirm").contains("Remove sentry?"));
  assert!(seeded.exists("connection-confirm-keep"), "Keep it beside the action");
  let bounds = |seeded: &mut Seeded, id: &str| {
    seeded
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (sends, question, tools) = (
    bounds(&mut seeded, "connection-variables"),
    bounds(&mut seeded, "connection-confirm"),
    bounds(&mut seeded, "connection-tools"),
  );
  assert!(
    sends.y + sends.height <= question.y && question.y + question.height <= tools.y,
    "the question sits between Sends and the tools: {sends:?} {question:?} {tools:?}"
  );
  seeded.click("connection-confirm-keep");
  assert!(!seeded.exists("connection-confirm"));
  assert!(seeded.server("sentry").is_some(), "Keep it keeps the server");
  seeded.click("connection-more");
  seeded.click("connection-remove");
  seeded.click("connection-confirm-remove");
  seeded.until("the server removed", |seeded| {
    seeded.server("sentry").is_none()
      && seeded.selected().is_none()
      && seeded.exists("connections-mcp")
      && !seeded.text("connections-mcp").contains("sentry")
  });
}

/// Confirming a save stores the fields as they are when it is pressed, also a value changed while the question
/// showed: the question says the same, so it is not shown anew.
#[test]
fn connections_mcp_server_confirmed_save_stores_the_current_fields() {
  let mut seeded = Seeded::new("connections");
  let docs = seeded.server("kontur-docs").expect("kontur-docs");
  seeded.go(ProjectPage::EditConnection(docs.id()));
  seeded.until("the form", |seeded| seeded.exists("connection-mcp-variables"));
  seeded.set("connection-mcp-variables", "DOCS_INDEX=first");
  seeded.click("connection-save");
  seeded.until("the question", |seeded| {
    seeded.exists("connection-confirm-save-confirm")
  });
  assert!(seeded.text("connection-confirm-save").contains("DOCS_INDEX"));
  seeded.set("connection-mcp-variables", "DOCS_INDEX=second");
  seeded.click("connection-confirm-save-confirm");
  seeded.until("the saved server", |seeded| seeded.exists("connection-inspector"));
  let edited = seeded.server("kontur-docs").expect("kontur-docs");
  let values: Vec<String> = edited
    .environment()
    .iter()
    .map(|(name, value)| format!("{name}={value:?}"))
    .collect();
  assert_eq!(
    values,
    [r#"DOCS_INDEX=Text("second")"#],
    "the value shown when confirming is stored"
  );
}
