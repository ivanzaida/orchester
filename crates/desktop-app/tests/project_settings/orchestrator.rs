//! Orchestrator (`S03` `X8mFTS`): the tab after Runs with *The Orchestrator may*'s six switches, each the project's own
//! and flipped only by a person, and *Let automation answer approvals* read-only with *Open* to Settings › Automation.
use orchester_app::routes::Route;
use orchester_workspace::workspace::OrchestratorConsent;

use super::harness::Settings;

/// Each switch's element id, its label and the consent it saves.
const SWITCHES: [(&str, &str, OrchestratorConsent); 6] = [
  (
    "settings-orchestrator-publish-drafts",
    "Publish drafts",
    OrchestratorConsent::PublishDrafts,
  ),
  (
    "settings-orchestrator-answer-agents",
    "Answer agents",
    OrchestratorConsent::AnswerAgents,
  ),
  (
    "settings-orchestrator-merge-runs",
    "Merge runs",
    OrchestratorConsent::MergeRuns,
  ),
  (
    "settings-orchestrator-merge-finished",
    "Merge finished runs on its own",
    OrchestratorConsent::MergeFinished,
  ),
  (
    "settings-orchestrator-continue-interrupted",
    "Continue interrupted runs",
    OrchestratorConsent::ContinueInterrupted,
  ),
  (
    "settings-orchestrator-write-git",
    "Write with git",
    OrchestratorConsent::WriteGit,
  ),
];

/// Project settings on its Orchestrator tab.
fn orchestrator() -> Settings {
  let mut page = Settings::new(false);
  page.tab("orchestrator", "settings-orchestrator-computer");
  page
}

fn left(page: &mut Settings, id: &str) -> f32 {
  page
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .x
}

#[test]
fn project_settings_orchestrator_is_the_tab_after_runs() {
  let mut page = orchestrator();
  let (runs, orchestrator, context) = (
    left(&mut page, "settings-tab-runs"),
    left(&mut page, "settings-tab-orchestrator"),
    left(&mut page, "settings-tab-context"),
  );
  assert!(
    runs < orchestrator && orchestrator < context,
    "{runs} {orchestrator} {context}"
  );
  assert!(
    page
      .text("settings-orchestrator-may-head")
      .contains("The Orchestrator may")
  );
  assert!(
    page
      .text("settings-orchestrator-may-head")
      .contains("Only you, here in the window")
  );
  assert!(
    page
      .text("settings-orchestrator-may-foot")
      .starts_with("Off: the Orchestrator proposes it")
  );
  // Merge finished runs on its own needs the two above it: it starts further in.
  let (merge, finished) = (
    left(&mut page, "settings-orchestrator-merge-runs-label"),
    left(&mut page, "settings-orchestrator-merge-finished-label"),
  );
  assert_eq!(finished - merge, 26.0);
}

#[test]
fn project_settings_orchestrator_switches_are_the_projects_and_only_a_persons() {
  let mut page = orchestrator();
  let workspace = page.window.launch.workspace().expect("session").clone();
  let repo = page.repo.clone();
  assert!(
    page.window.link.input_gate().protecting(),
    "MCP window input is refused"
  );
  for (switch, label, consent) in SWITCHES {
    page.until("the switch read", |p| p.exists(&format!("{switch}-mark")));
    assert!(page.text(&format!("{switch}-row")).contains(label));
    assert!(!page.switch_on(switch), "{label} is off until turned on");
    page.until("automation's change refused", |p| {
      lurq::app::events::with_synthetic_input(|| p.click(switch));
      p.exists(&format!("{switch}-blocked"))
    });
    assert!(!workspace.orchestrator_allows(&repo, consent).expect("read"));
    page.until("the person's choice saved for the project", |p| {
      let stored = workspace.orchestrator_allows(&repo, consent).expect("read");
      if !stored {
        p.click(switch);
      }
      stored
    });
    page.until("the switch to show it", |p| p.switch_on(switch));
    assert!(
      !workspace.app_wide_orchestrator_allows(consent).expect("read"),
      "{label} is saved for the project, not application-wide"
    );
  }
  page.saved();
  // Off holds in the project from the person's click, before the save runs.
  let (switch, _, consent) = SWITCHES[0];
  page.click(switch);
  assert!(!workspace.orchestrator_allows(&repo, consent).expect("read"));
}

#[test]
fn project_settings_orchestrator_shows_answering_approvals_and_opens_automation() {
  let mut page = Settings::new(false);
  let workspace = page.window.launch.workspace().expect("session").clone();
  workspace
    .set_automation_answers_approvals(true)
    .expect("on for this computer");
  page.tab("orchestrator", "settings-orchestrator-computer");
  page.until("its state", |p| {
    p.exists("settings-orchestrator-answer-approvals-value")
  });
  assert_eq!(page.text("settings-orchestrator-answer-approvals-value"), "On");
  assert!(
    page
      .text("settings-orchestrator-answer-approvals-hint")
      .contains("Settings › Automation")
  );
  page.click("settings-orchestrator-answer-approvals-open");
  page.until("Settings › Automation", |p| {
    p.window.navigation().route_now() == Route::Settings
  });
  assert!(
    workspace.automation_answers_approvals().expect("read"),
    "the row only reads it"
  );
}
