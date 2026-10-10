//! MCP servers as a person sets them up in the window: adding one through Add MCP server, changing its tools
//! that run without asking through Edit MCP server, turning it off (asked in place when profiles choose it)
//! and on, and removing one no profile chooses; one a profile chooses cannot be removed. A field that cannot
//! be used says why, and Save says the first reason.

use orchester_app::routes::{ConnectionsTab, NewConnection, ProjectPage};
use orchester_core::connection::mcp::McpTransport;

use super::harness::Seeded;

/// The program the user adds Pencil's MCP server with, on each platform.
#[cfg(windows)]
const PENCIL: &str = r"D:\Pencil\resources\app.asar.unpacked\out\mcp-server-windows-x64.exe";
#[cfg(not(windows))]
const PENCIL: &str = "/Applications/Pencil.app/Contents/Resources/app.asar.unpacked/out/mcp-server-darwin-arm64";
const TOOLS: &str = "get_editor_state\nget_guidelines\nbatch_get\nsnapshot_layout\nget_screenshot\nget_variables\nbatch_design\nset_variables\nexport_nodes";

#[test]
fn connections_mcp_server_is_added_with_its_tools_that_run_without_asking() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::McpServer);
  seeded.set("connection-mcp-name", "Pencil Design");
  assert!(
    seeded
      .text("connection-form-reason")
      .contains("Use small letters, digits"),
    "the first reason is the name's"
  );
  seeded.set("connection-mcp-name", "pencil-design");
  seeded.set("connection-mcp-program", PENCIL);
  seeded.set("connection-mcp-arguments", "--app\ndesktop\n--agent\ncodexCLI");
  seeded.set("connection-mcp-exempt-tools", TOOLS);
  assert!(
    !seeded.exists("connection-form-reason"),
    "nothing keeps it from being saved"
  );
  seeded.click("connection-save");
  seeded.until("the saved server", |seeded| seeded.exists("connection-inspector"));
  let server = seeded.server("pencil-design").expect("the server is stored");
  let McpTransport::Stdio(stdio) = server.transport() else {
    panic!("a program on this computer");
  };
  assert_eq!(stdio.program().as_str(), PENCIL);
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
  assert_eq!(seeded.text("connection-form-status"), "No changes");
  seeded.set("connection-mcp-exempt-tools", "search_docs");
  assert_eq!(seeded.text("connection-form-status"), "Unsaved changes");
  seeded.click("connection-save");
  seeded.until("the saved server", |seeded| seeded.exists("connection-inspector"));
  let edited = seeded.server("kontur-docs").expect("kontur-docs");
  assert_eq!(edited.exempt_tools().len(), 1);
  assert_eq!(edited.transport(), docs.transport(), "the program stays");
  assert_eq!(edited.environment(), docs.environment(), "the variables stay");
}

#[test]
fn connections_mcp_server_is_turned_off_and_on_and_removed() {
  let mut seeded = Seeded::new("connections");
  seeded.list(ConnectionsTab::McpServers);
  let docs = seeded.server("kontur-docs").expect("kontur-docs");
  seeded.click(&Seeded::row(docs.id()));
  // Designer chooses it: turning it off asks first.
  seeded.click("connection-turn-off");
  assert!(seeded.text("connection-confirm").contains("Turn off kontur-docs?"));
  seeded.click("connection-confirm-keep");
  assert!(!seeded.exists("connection-confirm"));
  assert!(seeded.server("kontur-docs").expect("kontur-docs").enabled());
  seeded.click("connection-turn-off");
  seeded.click("connection-confirm-turn-off");
  seeded.until("the server turned off", |seeded| {
    seeded.server("kontur-docs").is_some_and(|server| !server.enabled()) && seeded.exists("connection-turn-on")
  });
  seeded.click("connection-turn-on");
  seeded.until("the server turned on", |seeded| {
    seeded.server("kontur-docs").is_some_and(|server| server.enabled()) && seeded.exists("connection-turn-off")
  });
  let sentry = seeded.server("sentry").expect("sentry");
  seeded.click(&Seeded::row(sentry.id()));
  seeded.click("connection-remove");
  assert!(seeded.text("connection-confirm").contains("Remove sentry?"));
  seeded.click("connection-confirm-remove");
  seeded.until("the server removed", |seeded| {
    seeded.server("sentry").is_none()
      && seeded.selected().is_none()
      && seeded.text("connections-tab-mcp_servers") == "MCP servers 2"
  });
}
