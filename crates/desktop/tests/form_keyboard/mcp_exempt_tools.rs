//! Tools that run without asking: typed in the MCP server dialog, listed in its details, and named in
//! the profile's enforcement panel while the profile asks before MCP actions.

use orchester_core::{
  connection::{AgentProvider, mcp::ExemptTools},
  store::McpServerStore,
};
use orchester_store::ProjectDb;

use crate::{
  harness::Desktop,
  mcp_servers::{INSPECTOR, PRESSES, open_mcp_tab, seed_profile, seed_servers, shows},
};

const DIALOG: &str = "connection-dialog";
const FIELD: &str = "mcp-exempt-tools";

#[test]
fn mcp_exempt_tools_are_typed_one_per_line_and_listed_in_the_details() {
  let mut desktop = Desktop::with_project();
  let ids = seed_servers(desktop.repo());
  open_mcp_tab(&mut desktop);
  desktop.wait_for(INSPECTOR);
  assert!(shows(&mut desktop, INSPECTOR, "Tools that run without asking"));
  assert!(shows(&mut desktop, INSPECTOR, "every tool of this server asks"));
  desktop.click("edit-connection");
  desktop.wait_for(DIALOG);
  desktop.tab_to(FIELD, PRESSES);
  desktop.type_text("get screenshot");
  assert!(shows(&mut desktop, DIALOG, "get screenshot is not a tool name"));
  assert!(shows(&mut desktop, "connection-save-reason", "is not a tool name"));
  for _ in 0.."get screenshot".len() {
    desktop.key("Backspace", "Backspace");
  }
  desktop.type_text("get_screenshot");
  // Enter starts the next line of the area; it does not save.
  desktop.enter();
  desktop.type_text("batch_get");
  assert!(desktop.tree.get_element_by_id(DIALOG).is_some());
  desktop.press("connection-save");
  desktop.wait_until("the dialog closes", |desktop| {
    desktop.tree.get_element_by_id(DIALOG).is_none()
  });
  let db = ProjectDb::open(desktop.repo()).expect("project database");
  let stored = db.mcp_server(ids[0]).expect("read").expect("stored");
  assert_eq!(
    *stored.exempt_tools(),
    ExemptTools::parse("get_screenshot\nbatch_get").expect("valid")
  );
  desktop.wait_until("the details list the tools", |desktop| {
    shows(desktop, INSPECTOR, "batch_get")
  });
  assert!(shows(&mut desktop, INSPECTOR, "get_screenshot"));
  assert!(shows(&mut desktop, INSPECTOR, "Every other tool still asks"));
}

/// `docs` with two tools that run without asking, chosen by a profile on `provider`, its Permissions
/// tab open.
fn profile_with_exempt_docs(provider: AgentProvider) -> Desktop {
  let mut desktop = Desktop::with_project();
  let ids = seed_servers(desktop.repo());
  let mut db = ProjectDb::open(desktop.repo()).expect("project database");
  let mut docs = db.mcp_server(ids[0]).expect("read").expect("stored");
  docs.set_exempt_tools(ExemptTools::parse("search\nread_page").expect("valid"));
  db.save_mcp_server(&docs).expect("saved");
  seed_profile(desktop.repo(), provider, vec![ids[0]]);
  desktop.wait_for("nav-agents");
  desktop.click("nav-agents");
  desktop.wait_for("agent-role");
  desktop.press("agent-tab-permissions");
  desktop.wait_for("agent-mcp");
  desktop
}

/// The row shows only while the profile asks before MCP actions, with the CLI's mechanism.
#[test]
fn mcp_exempt_tools_are_named_in_the_enforcement_panel_when_the_profile_asks() {
  for (provider, mechanism) in [
    (AgentProvider::ClaudeCodeCli, "--allowedTools mcp__docs__<tool>"),
    (
      AgentProvider::CodexCli,
      "tools.<tool>.approval_mode = approve · other tools ask",
    ),
  ] {
    let mut desktop = profile_with_exempt_docs(provider);
    let row = "docs: 2 tools run without asking";
    assert!(!shows(&mut desktop, "agent-enforcement", row), "{provider:?}");
    desktop.tab_to("agent-tool-approvals", PRESSES);
    desktop.enter();
    assert!(shows(&mut desktop, "agent-enforcement", row), "{provider:?}");
    assert!(shows(&mut desktop, "agent-enforcement", mechanism), "{provider:?}");
    desktop.enter();
    assert!(!shows(&mut desktop, "agent-enforcement", row), "{provider:?}");
  }
}
