//! Permissions › Approvals (07, 07b): Autopilot or Manual with what each shows under it, turning Autopilot on asked in
//! place as a widening and named in the draft bar, switching back to Manual without a question.
use orchester_core::permission::ApprovalMode;

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

/// A profile stored before approval modes with *Approve automatically (don't ask me)* on reads as Autopilot: the page
/// shows Autopilot chosen and no Approve automatically row.
#[test]
fn agents_approvals_old_approving_automatically_shows_as_autopilot() {
  let mut screen = Agents::new(true);
  orchester_store::ProjectDb::open(&screen.root)
    .expect("database")
    .save_allow_autopilot(orchester_core::run::AllowAutopilot::ON)
    .expect("Autopilot allowed");
  let id = screen.profile_with(true, |permissions| {
    let mut stored = serde_json::to_value(&*permissions).expect("stored");
    stored["auto_approve"] = serde_json::json!(true);
    *permissions = serde_json::from_value(stored).expect("the old stored form reads");
  });
  assert_eq!(mode(&screen, id), ApprovalMode::Autopilot);
  screen.open(id);
  assert!(screen.exists("agent-still-asks"), "Autopilot's own rows show");
  assert!(!screen.exists("agent-auto-row"));
  assert!(
    !screen.text("agent-approvals").contains("Approve automatically"),
    "the old switch is gone"
  );
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
