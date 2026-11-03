//! Permissions › Approvals (07, 07b): Autopilot or Manual with what each shows under it, turning Autopilot on asked in
//! place as a widening and named in the draft bar, switching back to Manual without a question.
use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, EndpointPreset},
  id::{ConnectionId, SecretId},
  name::Name,
  permission::ApprovalMode,
  store::ConnectionStore,
  time::Timestamp,
};

use super::harness::Agents;

fn mode(screen: &Agents, id: orchester_core::id::ResourceId) -> ApprovalMode {
  screen
    .listed()
    .profile(id)
    .expect("profile")
    .profile
    .permissions()
    .approval_mode()
}

#[test]
fn agents_approvals_manual_lists_allowed_commands_and_network_stands_alone() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  assert!(screen.exists("agent-mode-manual-row") && screen.exists("agent-mode-autopilot-row"));
  assert!(screen.exists("agent-commands"), "Manual carries its allowed commands");
  assert!(screen.exists("agent-ask-tools"), "and asking before MCP actions");
  assert!(!screen.exists("agent-still-asks"));
  assert!(
    !screen.exists("agent-auto"),
    "the old Approve automatically switch is gone"
  );
  let decisions = screen.text("agent-approvals-decisions");
  assert!(decisions.contains("In both modes decisions come to you"), "{decisions}");
  assert_eq!(screen.text("agent-network-section-title"), "Network");
  assert!(
    !screen.exists("agent-option-AnyCommand"),
    "the commands moved to Approvals"
  );
}

/// A profile that approves automatically, set before approval modes or over MCP, runs so: it shows in Approvals while
/// it is on, and turning it off saves without asking.
#[test]
fn agents_approvals_shows_approving_automatically_while_it_is_on() {
  let mut screen = Agents::new(true);
  let id = screen.profile_with(true, |permissions| permissions.set_auto_approve(true));
  screen.open(id);
  let row = screen.text("agent-auto-row");
  assert!(row.contains("Approve automatically"), "{row}");
  assert!(
    screen.text("agent-approvals").contains("Approve automatically"),
    "inside the Approvals box"
  );
  screen.click("agent-auto");
  screen.until("turned off", |s| {
    assert!(!s.exists("agent-confirm-confirm"), "turning it off never asks");
    !s.listed()
      .profile(id)
      .expect("profile")
      .profile
      .permissions()
      .auto_approve()
  });
  assert!(!screen.exists("agent-auto-row"), "gone once off");
}

/// A new profile starts approving automatically; turned off before it is created, it stays off through publishing.
#[test]
fn agents_approvals_new_profile_turned_off_keeps_it_off() {
  let mut screen = Agents::new(true);
  screen.go();
  screen.click("agents-new");
  screen.until("the new profile's editor", |s| s.exists("agent-auto-row"));
  screen.click("agent-auto");
  screen.until("turned off", |s| !s.exists("agent-auto-row"));
  screen.set("agent-role", "Reviewer");
  screen.set("agent-instructions", "Review the implementation.");
  screen.set("agent-custom-model", "test-model");
  screen.click("agent-create");
  screen.until("created", |s| {
    s.listed().profiles.len() == 1 && s.exists("agent-publish")
  });
  screen.click("agent-publish");
  screen.until("first version", |s| {
    matches!(
      s.listed().profiles[0].state,
      orchester_workspace::agents::data::VersionState::Published(_)
    )
  });
  assert!(!screen.listed().profiles[0].profile.permissions().auto_approve());
  assert!(!screen.exists("agent-auto-row"));
}

/// A new profile on a native connection, which does not offer *Approve automatically*, takes the default when the
/// person picks a CLI connection: they never chose it off.
#[test]
fn agents_approvals_new_profile_moving_from_native_to_cli_approves_automatically() {
  let mut screen = Agents::new(true);
  let native = AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Anthropic API").expect("name"),
    AgentProvider::AnthropicApi,
    ApiEndpoint::new(
      EndpointPreset::Official,
      BaseUrl::new("https://api.anthropic.com").expect("url"),
      SecretId::generate(),
    ),
    Timestamp::now(),
  )
  .expect("native");
  orchester_store::ProjectDb::open(&screen.root)
    .expect("database")
    .create_connection(&native)
    .expect("store native");
  screen.go();
  screen.click("agents-new");
  screen.until("the new profile's editor", |s| s.exists("agent-role-row"));
  let pick = |screen: &mut Agents, name: &str| {
    screen.click("agent-connection");
    screen.until("the connections", |s| s.exists("agent-connection-option-0"));
    let option = (0..8)
      .map(|index| format!("agent-connection-option-{index}"))
      .find(|id| screen.exists(id) && screen.text(&format!("{id}-label")).contains(name))
      .unwrap_or_else(|| panic!("{name} option"));
    screen.click(&option);
  };
  pick(&mut screen, "Anthropic API");
  screen.until("native, without the switch", |s| !s.exists("agent-auto-row"));
  pick(&mut screen, "Test Codex");
  screen.until("Codex, approving automatically", |s| s.exists("agent-auto-row"));
  screen.set("agent-role", "Reviewer");
  screen.set("agent-instructions", "Review the implementation.");
  screen.click("agent-create");
  screen.until("created or asked", |s| {
    s.listed().profiles.len() == 1 || s.exists("agent-confirm-confirm")
  });
  // The native connection may leave the network broader than Codex's start, which is asked; approving automatically,
  // the default, never is.
  if screen.exists("agent-confirm-confirm") {
    let asked = screen.text("agent-confirm");
    assert!(!asked.contains("Approving automatically"), "{asked}");
    screen.window.tree.tick_futures();
    screen.window.pass();
    screen.click("agent-confirm-confirm");
  }
  screen.until("created", |s| s.listed().profiles.len() == 1);
  let saved = &screen.listed().profiles[0].profile;
  assert_ne!(saved.connection(), native.id());
  assert!(saved.permissions().auto_approve());
}

#[test]
fn agents_turning_autopilot_on_is_asked_in_place_and_named_in_the_draft_bar() {
  let mut screen = Agents::new(true);
  // 07b's project allows Autopilot; one that doesn't shows 02e instead (`approval_modes`).
  orchester_store::ProjectDb::open(&screen.root)
    .expect("database")
    .save_allow_autopilot(orchester_core::run::AllowAutopilot::ON)
    .expect("Autopilot allowed");
  let id = screen.profile(true);
  screen.open(id);
  screen.click("agent-mode-autopilot");
  let still = screen.text("agent-still-asks");
  for words in [
    "Still asks you",
    "Outside the worktree, Sources and temp folders",
    "Pushes to a remote",
    "Refs other than the run’s branch",
  ] {
    assert!(still.contains(words), "{still}");
  }
  assert!(!screen.exists("agent-commands"), "Autopilot needs no list");
  screen.until("question in Approvals", |s| s.exists("agent-confirm-confirm"));
  assert_eq!(screen.text("agent-confirm-title"), "Allow Autopilot for Builder?");
  assert!(
    screen.text("agent-approvals").contains("Allow Autopilot for Builder?"),
    "the question is the last row inside the Approvals box"
  );
  let bar = screen.text("agent-draft-bar");
  assert!(
    bar.contains("Approvals · Manual → Autopilot · waits for you to allow it"),
    "{bar}"
  );
  assert_eq!(
    mode(&screen, id),
    ApprovalMode::Manual,
    "nothing saved before the answer"
  );
  screen.window.tree.tick_futures();
  screen.window.pass();
  screen.click("agent-confirm-confirm");
  screen.until("Autopilot saved", |s| mode(s, id) == ApprovalMode::Autopilot);
  screen.saved();
  let bar = screen.text("agent-draft-bar");
  assert!(bar.contains("Approvals · Manual → Autopilot"), "{bar}");
  assert!(!bar.contains("waits for you"), "{bar}");
  // Switching back to Manual narrows: it saves without asking.
  screen.click("agent-mode-manual");
  screen.until("Manual saved", |s| {
    assert!(!s.exists("agent-confirm-confirm"), "Manual never asks");
    mode(s, id) == ApprovalMode::Manual
  });
}
