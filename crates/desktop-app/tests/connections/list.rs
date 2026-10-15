//! The list and its inspectors: what each tab lists and counts, what a row says, the inspector of the
//! chosen connection for each kind, and what the screen shows without connections, while it reads them, and
//! when reading fails.

use orchester_app::routes::{ConnectionsTab, ConnectionsView, ProjectPage};
use orchester_core::store::ConnectionStore;
use orchester_workspace::links::UsageLink;

use super::harness::Seeded;

#[test]
fn connections_tabs_list_and_count_each_kind() {
  let mut seeded = Seeded::new("connections");
  seeded.list(ConnectionsTab::All);
  for (tab, label) in [
    ("connections-tab-all", "All connections 6"),
    ("connections-tab-agent_providers", "Agent providers 2"),
    ("connections-tab-github", "GitHub 1"),
    ("connections-tab-mcp_servers", "MCP servers 3"),
  ] {
    assert_eq!(seeded.text(tab), label);
  }
  let list = seeded.text("connections-list");
  for words in [
    "Claude · Kontur",
    "Anthropic · Claude Code CLI 2.1.89",
    "Sign-in required",
    "Codex · Kontur",
    "5 agents",
    "GitHub · github.com",
    "gh · github.com · signed in as ivanzaida",
    "Program · npx",
    "Turned off",
    "Remote MCP · mcp.sentry.dev",
  ] {
    assert!(list.contains(words), "{words} in {list}");
  }
  seeded.list(ConnectionsTab::McpServers);
  let list = seeded.text("connections-list");
  assert!(
    list.contains("kontur-docs") && !list.contains("Codex · Kontur"),
    "{list}"
  );
  assert_eq!(seeded.text("connections-add"), "Add MCP server");
}

#[test]
fn connections_inspector_shows_the_chosen_connection_of_each_kind() {
  let mut seeded = Seeded::new("connections");
  seeded.list(ConnectionsTab::All);
  // The tab's first connection until one is chosen: Claude, its account not signed in, the fix in Settings.
  let inspector = seeded.text("connection-inspector");
  for words in [
    "Claude · Kontur",
    "Sign-in required",
    "Claude Code CLI",
    "claude-opus-5-5",
    "Open Default in Settings",
  ] {
    assert!(inspector.contains(words), "{words} in {inspector}");
  }
  let db = seeded.db();
  let github = db
    .connections()
    .expect("connections read")
    .into_iter()
    .find(|connection| connection.name().as_str() == "GitHub · github.com")
    .expect("the GitHub connection");
  seeded.click(&Seeded::row(github.id()));
  assert_eq!(seeded.selected(), Some(github.id()));
  let inspector = seeded.text("connection-inspector");
  for words in ["Signed in", "gh 2.62.0", "ivanzaida", "repo, read:org", "Check again"] {
    assert!(inspector.contains(words), "{words} in {inspector}");
  }
  let sentry = seeded.server("sentry").expect("sentry");
  seeded.click(&Seeded::row(sentry.id()));
  let inspector = seeded.text("connection-inspector");
  for words in [
    "Remote MCP",
    "https://mcp.sentry.dev/mcp",
    "Bearer token from SENTRY_TOKEN (development)",
    "Turn off",
  ] {
    assert!(inspector.contains(words), "{words} in {inspector}");
  }
  let docs = seeded.server("kontur-docs").expect("kontur-docs");
  seeded.click(&Seeded::row(docs.id()));
  let inspector = seeded.text("connection-inspector");
  for words in [
    "npx",
    "-y @kontur/docs-mcp --root docs",
    "DOCS_INDEX",
    "search_docs",
    "get_page",
    "Designer",
  ] {
    assert!(inspector.contains(words), "{words} in {inspector}");
  }
  assert!(
    seeded
      .text("connection-inspector-footer")
      .contains("Remove is unavailable while agent profiles choose this server: Designer."),
    "a server a profile chooses cannot be removed"
  );
}

#[test]
fn connections_without_any_say_what_a_connection_is_and_offer_each_kind() {
  let mut seeded = Seeded::new("empty");
  seeded.list(ConnectionsTab::All);
  let empty = seeded.text("connections-empty");
  assert!(empty.contains("No connections yet"), "{empty}");
  for action in [
    "connections-empty-agent",
    "connections-empty-github",
    "connections-empty-mcp",
  ] {
    assert!(seeded.exists(action), "{action}");
  }
  assert_eq!(seeded.text("connections-tab-all"), "All connections 0");
  seeded.list(ConnectionsTab::McpServers);
  assert!(seeded.text("connections-empty-tab").contains("No MCP servers yet"));
}

#[test]
fn connections_say_they_are_read_and_why_reading_failed() {
  let mut seeded = Seeded::new("connections");
  seeded.go(ProjectPage::Connections(ConnectionsView::default()));
  assert!(
    seeded.exists("connections-loading"),
    "the first pass reads the connections"
  );
  seeded.until("the connections", |seeded| seeded.exists("connections-body"));
  let sqlite =
    rusqlite::Connection::open(seeded.repo.join(".orchester").join("orchester.db")).expect("the database opens");
  sqlite.execute("DROP TABLE mcp_servers", []).expect("the table goes");
  // Coming back reads them again.
  seeded.go(ProjectPage::Usage(UsageLink::default()));
  seeded.go(ProjectPage::Connections(ConnectionsView::new(ConnectionsTab::GitHub)));
  seeded.until("the failure", |seeded| seeded.exists("connections-load-error"));
  assert!(
    seeded
      .text("connections-load-error")
      .contains("Connections could not be read")
  );
}
