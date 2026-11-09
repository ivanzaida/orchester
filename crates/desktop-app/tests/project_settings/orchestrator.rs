//! Orchestrator (`S03` `X8mFTS`): the tab after Runs with *Let the Orchestrator drive*, the project's own switch,
//! flipped only by a person (ORC-909: it replaced six), and *Let automation answer approvals* read-only with *Open* to
//! Settings › Automation.
use orchester_app::routes::Route;

use super::harness::Settings;

/// The switch's element id and its label.
const SWITCH: &str = "settings-orchestrator-drive";
const LABEL: &str = "Let the Orchestrator drive";

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
  assert!(page.text("settings-orchestrator-may-head").contains("Driving"));
  assert!(
    page
      .text("settings-orchestrator-may-head")
      .contains("Only you, here in the window")
  );
  assert!(
    page
      .text("settings-orchestrator-may-foot")
      .starts_with("Off: the Orchestrator proposes each change")
  );
}

#[test]
fn project_settings_orchestrator_has_one_switch_which_is_the_projects_and_only_a_persons() {
  let mut page = orchestrator();
  let workspace = page.window.launch.workspace().expect("session").clone();
  let repo = page.repo.clone();
  assert!(
    page.window.link.input_gate().protecting(),
    "MCP window input is refused"
  );
  for old in [
    "publish-drafts",
    "answer-agents",
    "merge-runs",
    "merge-finished",
    "continue-interrupted",
    "write-git",
  ] {
    assert!(
      !page.exists(&format!("settings-orchestrator-{old}-row")),
      "the {old} switch is folded into Let the Orchestrator drive"
    );
  }
  page.until("the switch read", |p| p.exists(&format!("{SWITCH}-mark")));
  assert!(page.text(&format!("{SWITCH}-row")).contains(LABEL));
  assert!(!page.switch_on(SWITCH), "{LABEL} is off until turned on");
  page.until("automation's change refused", |p| {
    lurq::app::events::with_synthetic_input(|| p.click(SWITCH));
    p.exists(&format!("{SWITCH}-blocked"))
  });
  assert!(!workspace.orchestrator_drives(&repo).expect("read"));
  page.until("the person's choice saved for the project", |p| {
    let stored = workspace.orchestrator_drives(&repo).expect("read");
    if !stored {
      p.click(SWITCH);
    }
    stored
  });
  page.until("the switch to show it", |p| p.switch_on(SWITCH));
  page.until("what it does said", |p| p.exists(&format!("{SWITCH}-on")));
  page.saved();
  // Off holds in the project from the person's click, before the save runs.
  page.click(SWITCH);
  assert!(!workspace.orchestrator_drives(&repo).expect("read"));
}

#[test]
fn project_settings_orchestrator_shows_answering_approvals_and_opens_automation() {
  let mut page = Settings::new(false);
  let workspace = page.window.launch.workspace().expect("session").clone();
  workspace
    .set_automation_answers_approvals(true)
    .expect("on for this computer");
  page.tab("orchestrator", "settings-orchestrator-computer");
  // The workspace counted the change, which reads Settings › Automation's query again.
  page.until("its state", |p| {
    p.exists("settings-orchestrator-answer-approvals-value")
      && p.text("settings-orchestrator-answer-approvals-value") == "On"
  });
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
