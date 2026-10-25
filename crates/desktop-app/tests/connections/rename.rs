//! Renaming a connection (GitHub issue 54): an agent CLI or GitHub connection in its inspector's header, an MCP server
//! in Edit MCP server. A name is checked as on adding (not blank, no other connection of the project has it), and what
//! uses the connection names it by its id, so it keeps using it.

use orchester_app::routes::{ConnectionsTab, ConnectionsView, ProjectPage};
use orchester_core::{
  connection::{AgentProvider, ConnectionKind},
  id::ConnectionId,
  store::ConnectionStore,
};
use orchester_workspace::connections::data::ProjectConnections;

use super::harness::Seeded;

/// The connection of `kind` the seed has.
fn seeded_connection(seeded: &Seeded, kind: &ConnectionKind) -> ConnectionId {
  seeded
    .db()
    .connections()
    .expect("connections")
    .into_iter()
    .find(|connection| connection.kind() == kind)
    .expect("the seed's connection")
    .id()
}

fn name_of(seeded: &Seeded, id: ConnectionId) -> String {
  seeded
    .db()
    .connection(id)
    .expect("read")
    .expect("stored")
    .name()
    .as_str()
    .to_owned()
}

fn uses(seeded: &Seeded, id: ConnectionId) -> usize {
  ProjectConnections::load_in(&seeded.repo)
    .expect("the connections read")
    .used_by(id)
    .count()
}

/// Opens the rename of connection `id` on `tab`.
fn rename(seeded: &mut Seeded, tab: ConnectionsTab, id: ConnectionId) {
  seeded.go(ProjectPage::Connections(ConnectionsView::new(tab).with_selected(id)));
  seeded.until("the rename", |seeded| seeded.exists("connection-rename"));
  seeded.click("connection-rename");
  seeded.until("the name field", |seeded| seeded.exists("connection-rename-name"));
}

#[test]
fn connections_agent_connection_is_renamed_in_its_inspector_and_keeps_its_profiles() {
  let mut seeded = Seeded::new("connections");
  let codex = seeded_connection(&seeded, &ConnectionKind::Agent(AgentProvider::CodexCli));
  let profiles = uses(&seeded, codex);
  assert!(profiles > 0, "the seed's profiles run on Codex");
  rename(&mut seeded, ConnectionsTab::AgentProviders, codex);
  seeded.set("connection-rename-name", "Claude · Kontur");
  assert!(
    seeded
      .text("connection-rename-name-note")
      .contains("already has this name")
  );
  seeded.click("connection-rename-save");
  seeded.set("connection-rename-name", "   ");
  assert!(seeded.exists("connection-rename-name-note"), "a blank name is refused");
  seeded.click("connection-rename-save");
  assert_eq!(name_of(&seeded, codex), "Codex · Kontur", "nothing refused is stored");
  seeded.set("connection-rename-name", "  Codex · main ");
  assert!(!seeded.exists("connection-rename-name-note"));
  seeded.click("connection-rename-save");
  seeded.until("the new name stored", |seeded| name_of(seeded, codex) == "Codex · main");
  seeded.until("the inspector to show it", |seeded| {
    seeded.exists("connection-inspector-title") && seeded.text("connection-inspector-title") == "Codex · main"
  });
  assert!(seeded.text(&Seeded::row(codex)).contains("Codex · main"));
  assert_eq!(uses(&seeded, codex), profiles, "its profiles still run on it");
}

#[test]
fn connections_rename_is_cancelled_without_storing() {
  let mut seeded = Seeded::new("connections");
  let codex = seeded_connection(&seeded, &ConnectionKind::Agent(AgentProvider::CodexCli));
  rename(&mut seeded, ConnectionsTab::AgentProviders, codex);
  seeded.set("connection-rename-name", "Codex · elsewhere");
  seeded.click("connection-rename-cancel");
  assert!(!seeded.exists("connection-rename-name"));
  assert_eq!(seeded.text("connection-inspector-title"), "Codex · Kontur");
  assert_eq!(name_of(&seeded, codex), "Codex · Kontur");
}

#[test]
fn connections_github_connection_is_renamed_in_its_inspector() {
  let mut seeded = Seeded::new("connections");
  let github = seeded
    .db()
    .connections()
    .expect("connections")
    .into_iter()
    .find(|connection| matches!(connection.kind(), ConnectionKind::GitHub { .. }))
    .expect("the seed's GitHub connection")
    .id();
  rename(&mut seeded, ConnectionsTab::GitHub, github);
  seeded.set("connection-rename-name", "Codex · Kontur");
  assert!(
    seeded.exists("connection-rename-name-note"),
    "agent and GitHub connections share their names"
  );
  seeded.set("connection-rename-name", "GitHub · work");
  seeded.click("connection-rename-save");
  seeded.until("the new name stored", |seeded| {
    name_of(seeded, github) == "GitHub · work"
  });
  seeded.until("the inspector to show it", |seeded| {
    seeded.exists("connection-inspector-title") && seeded.text("connection-inspector-title") == "GitHub · work"
  });
}

#[test]
fn connections_mcp_server_is_renamed_in_its_form_and_keeps_its_profiles() {
  let mut seeded = Seeded::new("connections");
  let docs = seeded.server("kontur-docs").expect("kontur-docs");
  let profiles = uses(&seeded, docs.id());
  assert!(profiles > 0, "a seed profile gives its agents kontur-docs");
  seeded.go(ProjectPage::EditConnection(docs.id()));
  seeded.until("the form", |seeded| seeded.exists("connection-mcp-name"));
  seeded.set("connection-mcp-name", "postgres-dev");
  assert!(seeded.exists("connection-form-reason"), "another server has the name");
  seeded.set("connection-mcp-name", "kontur-handbook");
  seeded.click("connection-save");
  seeded.until("the server saved", |seeded| {
    if seeded.exists("connection-confirm-save-confirm") {
      seeded.click("connection-confirm-save-confirm");
    }
    seeded.exists("connection-inspector")
  });
  let renamed = seeded.server("kontur-handbook").expect("renamed");
  assert_eq!(renamed.id(), docs.id());
  assert_eq!(uses(&seeded, docs.id()), profiles, "its profiles still choose it");
}
