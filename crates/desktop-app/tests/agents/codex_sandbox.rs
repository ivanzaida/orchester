//! *Codex sandbox* on a profile's page (ORC-789): a Codex connection's profile has the switch, on by default, and
//! turning it off warns that Codex then enforces neither repository access, network nor writable folders, which
//! Connection enforcement then reads as *Not enforced*; a Claude Code profile has no such switch.
//! `cargo test -p orchester-app --test agents codex_sandbox`.
use orchester_core::{
  connection::AgentConnection,
  id::ConnectionId,
  name::Name,
  permission::{AgentPermissions, CodexSandbox},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::harness::Agents;

/// Write access, so that the folders row says what grants them.
fn writing(permissions: &mut AgentPermissions) {
  permissions.set_repository(orchester_core::permission::RepositoryAccess::Write);
}

fn assert_enforced(screen: &mut Agents, enforced: bool) {
  for row in [
    "agent-enforcement-repository",
    "agent-enforcement-folders",
    "agent-enforcement-network",
  ] {
    let text = screen.text(row);
    assert_eq!(text.contains("Not enforced"), !enforced, "{row}: {text}");
    assert_eq!(text.contains("sandbox off"), !enforced, "{row}: {text}");
  }
}

#[test]
fn codex_sandbox_switch_is_on_by_default_and_off_warns_and_is_not_enforced() {
  let mut screen = Agents::new(true);
  let id = screen.profile_with(true, writing);
  screen.open(id);
  let row = screen.text("agent-codex-sandbox-row");
  assert!(row.contains("Codex sandbox") && row.contains("On"), "{row}");
  assert!(!screen.exists("agent-codex-sandbox-warning"));
  let repository = screen.text("agent-enforcement-repository");
  assert!(repository.contains("sandbox · workspace-write"), "{repository}");
  assert!(!repository.contains("Not enforced"), "{repository}");

  screen.click("agent-codex-sandbox");
  assert!(screen.text("agent-codex-sandbox-row").contains("Off"));
  let warning = screen.text("agent-codex-sandbox-warning");
  assert!(
    warning.contains("Codex enforces neither repository access, network nor writable folders"),
    "{warning}"
  );
  assert_enforced(&mut screen, false);
}

#[test]
fn codex_sandbox_off_profile_reads_not_enforced() {
  let mut screen = Agents::new(true);
  let id = screen.profile_with(true, |permissions| {
    writing(permissions);
    permissions.set_codex_sandbox(CodexSandbox::Off);
  });
  screen.open(id);
  assert!(screen.text("agent-codex-sandbox-row").contains("Off"));
  assert!(screen.exists("agent-codex-sandbox-warning"));
  assert_enforced(&mut screen, false);
}

#[test]
fn codex_sandbox_switch_is_only_on_a_codex_connection() {
  let mut screen = Agents::new(false);
  let claude = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude CLI").expect("name"),
    orchester_core::connection::AgentProvider::ClaudeCodeCli,
    screen.root.join("claude"),
    Timestamp::now(),
  )
  .expect("connection");
  ProjectDb::open(&screen.root)
    .expect("db")
    .create_connection(&claude)
    .expect("store connection");
  let id = screen.profile(true);
  screen.open(id);
  assert!(screen.exists("agent-network-section"), "the permissions are shown");
  for gone in ["agent-sandbox", "agent-codex-sandbox-row"] {
    assert!(!screen.exists(gone), "{gone}");
  }
}
