//! MCP servers as a person sets them up in the window: changing a server's tools that run without asking through
//! Edit MCP server, turning it off (asked in place when profiles choose it) and on, and removing one no profile
//! chooses; one a profile chooses cannot be removed. Adding one is the Agents hub's form in a local project
//! (`tests/agents/local_hub_create*.rs`).

use orchester_app::routes::ProjectPage;

use super::harness::Seeded;

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
    seeded.server("sentry").is_none() && seeded.selected().is_none() && seeded.exists("connections-list")
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
