//! A profile's approval mode on its connection and in its project (`docs/design/approval-modes.md`): the Approval mode
//! row of Connection enforcement (07d), Enforced where the CLI hands its requests to Orchester and *Not enforced* with
//! the reason where it runs Autopilot as Manual; and a profile set to Autopilot in a project that doesn't allow it
//! (02e), which keeps its choice and says that Manual is used there. `cargo test -p orchester-app --test agents
//! approval_mode`.
use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};
use orchester_core::{
  permission::{ApprovalMode, CODEX_AUTOPILOT},
  run::AllowAutopilot,
};
use orchester_store::ProjectDb;

use super::{harness::Agents, support::project_id};

/// A Codex version that hands every request to Orchester.
const CURRENT_CODEX: &str = "0.159.2";

fn autopilot(screen: &Agents) -> orchester_core::id::ResourceId {
  screen.profile_with(true, |p| p.set_approval_mode(ApprovalMode::Autopilot))
}

#[test]
fn approval_mode_enforcement_row_says_how_the_cli_hands_requests_in_either_mode() {
  let mut screen = Agents::on_codex(CURRENT_CODEX);
  let manual = screen.profile(true);
  screen.open(manual);
  let row = screen.text("agent-enforcement-approvals");
  for words in [
    "Approval mode",
    "Manual · Orchester answers: allowed list, then the reviewer, then you",
    "Enforced",
  ] {
    assert!(row.contains(words), "{words}: {row}");
  }
  let pilot = autopilot(&screen);
  screen.open(pilot);
  let row = screen.text("agent-enforcement-approvals");
  for words in [
    "Approval mode",
    "Autopilot · Orchester answers every request; Codex’s own prompt is off",
    "Enforced",
  ] {
    assert!(row.contains(words), "{words}: {row}");
  }
  assert!(!row.contains("Not enforced"), "{row}");
}

#[test]
fn approval_mode_enforcement_row_runs_autopilot_as_manual_with_the_reason_on_an_older_cli() {
  // `Agents::new` checks Codex 0.139.0, older than the one Autopilot needs.
  assert!(orchester_core::connection::compare_versions(Some("0.139.0"), Some(CODEX_AUTOPILOT)).is_lt());
  let mut screen = Agents::new(true);
  let pilot = autopilot(&screen);
  screen.open(pilot);
  let row = screen.text("agent-enforcement-approvals");
  for words in [
    "Autopilot · Can’t hand its requests to Orchester · runs as Manual until Codex is updated",
    "Not enforced",
  ] {
    assert!(row.contains(words), "{words}: {row}");
  }
  // Manual starts as before on any version.
  let manual = screen.profile(true);
  screen.open(manual);
  let row = screen.text("agent-enforcement-approvals");
  assert!(row.contains("Manual · ") && !row.contains("Not enforced"), "{row}");
}

#[test]
fn approval_mode_autopilot_profile_in_a_project_that_doesnt_allow_it_says_manual_is_used_there() {
  let mut screen = Agents::on_codex(CURRENT_CODEX);
  let pilot = autopilot(&screen);
  screen.open(pilot);
  assert_eq!(screen.text("agent-mode-autopilot-not-allowed"), "Not allowed in Kontur");
  let hint = screen.text("agent-mode-autopilot-hint");
  assert!(
    hint.contains("Kontur doesn’t allow Autopilot, so Builder’s runs there use Manual."),
    "{hint}"
  );
  assert_eq!(screen.text("agent-mode-manual-used"), "Used in Kontur");
  let hint = screen.text("agent-mode-manual-hint");
  assert!(hint.starts_with("What Builder’s runs use in Kontur now"), "{hint}");
  assert!(
    !screen.exists("agent-still-asks"),
    "nothing of Autopilot applies in Kontur"
  );
  assert!(!screen.exists("agent-commands"), "the profile keeps its choice");
  assert_eq!(screen.text("agent-open-project-runs"), "Open Project settings › Runs");
  screen.click("agent-open-project-runs");
  assert_eq!(
    screen.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Settings(ProjectSettingsTab::Runs))
  );
}

#[test]
fn approval_mode_autopilot_profile_in_a_project_that_allows_it_shows_no_refusal() {
  let mut screen = Agents::on_codex(CURRENT_CODEX);
  ProjectDb::open(&screen.root)
    .expect("database")
    .save_allow_autopilot(AllowAutopilot::ON)
    .expect("Autopilot allowed");
  let pilot = autopilot(&screen);
  screen.open(pilot);
  assert!(screen.exists("agent-still-asks"));
  for id in [
    "agent-mode-autopilot-not-allowed",
    "agent-mode-manual-used",
    "agent-open-project-runs",
  ] {
    assert!(!screen.exists(id), "{id}");
  }
  let hint = screen.text("agent-mode-autopilot-hint");
  assert!(hint.starts_with("Approves every tool request at once."), "{hint}");
}
