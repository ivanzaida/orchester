//! Connections, a tab of Project settings (`05 Connections` `dk46o`): its groups and what a row says, a connection's
//! own page with the crumb back for each kind (`05b` `PGwka`, `13` `Ptkxo`, `14` `KItiZ`), and what the tab shows
//! without connections (`07` `WYCDL`), while it reads them, and when reading fails.

use orchester_app::routes::{ConnectionsView, NewConnection, ProjectPage};
use orchester_core::store::ConnectionStore;

use super::harness::Seeded;

#[test]
fn connections_groups_list_each_kind_with_its_add() {
  let mut seeded = Seeded::new("connections");
  seeded.list();
  for (group, words) in [
    (
      "connections-agents",
      &[
        "Agent providers",
        "Claude · Kontur",
        "Sign-in required",
        "Codex · Kontur",
        "5 agents",
      ][..],
    ),
    (
      "connections-github",
      &["GitHub", "gh · github.com", "signed in as ivanzaida"][..],
    ),
    (
      "connections-mcp",
      &["MCP servers", "kontur-docs", "sentry", "mcp.sentry.dev"][..],
    ),
  ] {
    let text = seeded.text(group);
    for word in words {
      assert!(text.contains(word), "{word} in {group}: {text}");
    }
  }
  assert_eq!(seeded.text("connections-add-agent"), "Add provider");
  assert_eq!(seeded.text("connections-add-mcp"), "Add MCP server");
  assert!(
    !seeded.exists("project-settings-save-status"),
    "Connections saves per action"
  );
}

/// Add provider says at the title's right that the provider isn't saved yet (`Saved` `v6VzvO` in `10` `LaTB4`).
#[test]
fn connections_add_provider_says_it_is_not_saved_yet() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::AgentProvider);
  assert_eq!(seeded.text("project-settings-save-status"), "Not saved yet");
}

/// A row opens the connection's page under the tab, which leads back to Connections by its crumb.
#[test]
fn connections_row_opens_the_connections_page_with_a_crumb_back() {
  let mut seeded = Seeded::new("connections");
  seeded.list();
  let db = seeded.db();
  let claude = db
    .connections()
    .expect("connections read")
    .into_iter()
    .find(|connection| connection.name().as_str() == "Claude · Kontur")
    .expect("the Claude connection");
  seeded.click(&Seeded::row(claude.id()));
  assert_eq!(seeded.selected(), Some(claude.id()));
  seeded.until("its page", |seeded| seeded.exists("connection-inspector"));
  assert_eq!(seeded.text("connection-crumb-back"), "Connections");
  let page = seeded.text("connection-inspector");
  for words in [
    "Claude · Kontur",
    "Sign-in required",
    "Open Default in Settings",
    "claude-opus-5-5",
  ] {
    assert!(page.contains(words), "{words} in {page}");
  }
  assert!(
    seeded.exists("settings-tab-connections"),
    "the page is under Project settings' tabs"
  );
  seeded.click("connection-more");
  for item in ["connection-rename", "connection-edit"] {
    assert!(seeded.exists(item), "{item} in the … menu");
  }
  seeded.click("connection-more");
  seeded.click("connection-crumb-back");
  assert_eq!(seeded.selected(), None);
  seeded.until("the list", |seeded| seeded.exists("connections-list"));

  let github = db
    .connections()
    .expect("connections read")
    .into_iter()
    .find(|connection| connection.name().as_str() == "gh · github.com")
    .expect("the GitHub connection");
  seeded.open(github.id());
  let page = seeded.text("connection-inspector");
  for words in ["Signed in", "gh 2.62.0", "ivanzaida", "repo, read:org", "Check again"] {
    assert!(page.contains(words), "{words} in {page}");
  }
  let sentry = seeded.server("sentry").expect("sentry");
  seeded.open(sentry.id());
  let page = seeded.text("connection-inspector");
  for words in ["https://mcp.sentry.dev/mcp", "SENTRY_TOKEN (development)"] {
    assert!(page.contains(words), "{words} in {page}");
  }
  let docs = seeded.server("kontur-docs").expect("kontur-docs");
  seeded.open(docs.id());
  let page = seeded.text("connection-inspector");
  for words in [
    "npx",
    "-y @kontur/docs-mcp --root docs",
    "DOCS_INDEX",
    "search_docs",
    "get_page",
    "Designer",
  ] {
    assert!(page.contains(words), "{words} in {page}");
  }
}

/// Without connections the tab says how to connect an agent: each CLI as found on the search path, a CLI on an API
/// provider, GitHub not linked with why, and MCP servers with their Add.
#[test]
fn connections_without_any_offer_each_way_to_connect() {
  let mut seeded = Seeded::new("empty");
  seeded.list();
  seeded.until("the CLIs looked for", |seeded| {
    seeded.exists("connections-empty") && !seeded.text("connections-agents").contains("Looking")
  });
  let empty = seeded.text("connections-empty");
  for words in ["Claude Code CLI", "Codex CLI", "No MCP servers yet"] {
    assert!(empty.contains(words), "{words} in {empty}");
  }
  for action in [
    "connections-way-api-add",
    "connections-add-reason",
    "connections-mcp-empty-add",
  ] {
    assert!(seeded.exists(action), "{action}");
  }
}

#[test]
fn connections_say_they_are_read_and_why_reading_failed() {
  let mut seeded = Seeded::new("connections");
  seeded.go(ProjectPage::Connections(ConnectionsView::default()));
  assert!(
    seeded.exists("connections-loading"),
    "the first pass reads the connections"
  );
  seeded.until("the connections", |seeded| seeded.exists("connections-list"));
  let sqlite =
    rusqlite::Connection::open(seeded.repo.join(".orchester").join("orchester.db")).expect("the database opens");
  sqlite.execute("DROP TABLE mcp_servers", []).expect("the table goes");
  // Coming back reads them again.
  seeded.go(ProjectPage::Usage(Box::default()));
  seeded.go(ProjectPage::Connections(ConnectionsView::default()));
  seeded.until("the failure", |seeded| seeded.exists("connections-load-error"));
  assert!(
    seeded
      .text("connections-load-error")
      .contains("Connections could not be read")
  );
}
