//! Test connection says in place what it does and what it found (GitHub issue 55): pressed, the inspector shows that
//! it is checking and how long the CLI has to answer, the button busy; then whether the test passed, what it found
//! and when, which stays until the next check; or that it failed and why, the footer's sentence saying what to do.

use orchester_app::{
  routes::{ConnectionsTab, ConnectionsView, ProjectPage},
  ui::icon::Icon,
};
use orchester_core::{
  connection::AgentConnection, connection::AgentProvider, id::ConnectionId, name::Name, store::ConnectionStore,
  time::Timestamp,
};

use super::{add_cli::write_codex, harness::Seeded};

/// A Codex connection named `name` whose program is `program`, never checked, in the inspector.
fn inspect(seeded: &mut Seeded, name: &str, program: std::path::PathBuf) -> ConnectionId {
  let connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(name).expect("a name"),
    AgentProvider::CodexCli,
    program,
    Timestamp::now(),
  )
  .expect("a connection");
  let id = connection.id();
  seeded
    .db()
    .create_connection(&connection.into_connection())
    .expect("created");
  let view = ConnectionsView::new(ConnectionsTab::AgentProviders).with_selected(id);
  seeded.go(ProjectPage::Connections(view));
  seeded.until("the inspector", |seeded| seeded.exists("connection-test"));
  id
}

#[test]
fn connections_test_shows_it_is_checking_then_what_passed() {
  let mut seeded = Seeded::new("connections");
  let folder = tempfile::tempdir().expect("a temporary folder");
  let program = write_codex(folder.path(), "999.0.0");
  inspect(&mut seeded, "Codex · stand-in", program);
  assert!(!seeded.exists("connection-test-result"), "nothing was tested here yet");
  seeded.click("connection-test");
  assert_eq!(seeded.text("connection-test-result-heading"), "Checking…");
  assert!(
    seeded
      .text("connection-test-result-detail")
      .contains("within 30 seconds"),
    "the time the CLI has to answer is said"
  );
  let glyph = seeded
    .window
    .tree
    .get_element_by_id("connection-test-icon")
    .and_then(|icon| icon.text_content().map(str::to_owned));
  assert_eq!(
    glyph,
    Some(Icon::LoaderCircle.glyph().to_string()),
    "the button is busy"
  );
  seeded.until("the test to pass", |seeded| {
    seeded.text("connection-test-result-heading") == "Test passed"
  });
  assert_eq!(
    seeded.text("connection-test-result-detail"),
    "Codex CLI 999.0.0 answers and is signed in. Models listed: 0."
  );
  assert!(seeded.exists("connection-test-result-time"), "when it was tested");
  assert!(seeded.text("connection-inspector-status").contains("Connected"));
}

#[test]
fn connections_test_that_fails_says_why_and_stays() {
  let mut seeded = Seeded::new("connections");
  let folder = tempfile::tempdir().expect("a temporary folder");
  // A program file that is not there; a missing command script would be run by cmd.exe, which answers itself.
  let program = if cfg!(windows) { "codex.exe" } else { "codex" };
  inspect(&mut seeded, "Codex · gone", folder.path().join(program));
  seeded.click("connection-test");
  assert_eq!(seeded.text("connection-test-result-heading"), "Checking…");
  seeded.until("the test to fail", |seeded| {
    seeded.text("connection-test-result-heading") == "Test failed"
  });
  assert_eq!(
    seeded.text("connection-test-result-detail"),
    "The program file was not found."
  );
  assert!(
    seeded
      .text("connection-inspector-sentence")
      .contains("Install the CLI again"),
    "the footer says what to do"
  );
  seeded.until("the window settled", |seeded| !seeded.window.tree.needs_redraw());
  assert_eq!(
    seeded.text("connection-test-result-heading"),
    "Test failed",
    "the result stays until the next check"
  );
}
