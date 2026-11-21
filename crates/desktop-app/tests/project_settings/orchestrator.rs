//! Orchestrator (`S03` `X8mFTS`): the tab after Runs with *Let the Orchestrator drive*, the project's own switch,
//! flipped only by a person (ORC-909: it replaced six). The tab no longer shows *Also needed · set for this computer*:
//! the chat does not depend on *Let automation answer approvals* (OA-11, `S01` `pK4S5`).
use super::harness::Settings;

/// The switch's element id and its label.
const SWITCH: &str = "settings-orchestrator-drive";
const LABEL: &str = "Let the Orchestrator drive";

/// Project settings on its Orchestrator tab.
fn orchestrator() -> Settings {
  let mut page = Settings::new(false);
  page.tab("orchestrator", "settings-orchestrator-leading");
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
  page.until("the switch read", |p| p.exists(&format!("{SWITCH}-track")));
  assert!(page.text(&format!("{SWITCH}-row")).contains(LABEL));
  assert!(!page.switch_on(SWITCH), "{LABEL} is off until turned on");
  assert!(
    page.window.link.input_gate().protecting(),
    "the gate refuses an MCP client's window input while the row shows"
  );
  // Synthetic input that reaches the switch all the same (a test's own) is only logged: nothing changes.
  for _ in 0..5 {
    lurq::app::events::with_synthetic_input(|| page.click(SWITCH));
  }
  assert!(!page.exists(&format!("{SWITCH}-blocked")));
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
fn project_settings_orchestrator_no_longer_shows_also_needed_for_this_computer() {
  let mut page = Settings::new(false);
  let workspace = page.window.launch.workspace().expect("session").clone();
  // Whatever the computer's setting says, the tab says nothing of it: it is for outside MCP clients alone.
  workspace
    .set_automation_answers_approvals(true)
    .expect("on for this computer");
  page.tab("orchestrator", "settings-orchestrator-leading");
  page.until("the tab read", |p| p.exists("settings-orchestrator-leading-runs-field"));
  for gone in [
    "settings-orchestrator-computer",
    "settings-orchestrator-answer-approvals-row",
    "settings-orchestrator-answer-approvals-open",
  ] {
    assert!(!page.exists(gone), "{gone} is gone from the tab");
  }
  let shown = page.text("screen-project_settings");
  for words in [
    "Also needed",
    "Let automation answer approvals",
    "set for this computer",
  ] {
    assert!(!shown.contains(words), "{words}: {shown}");
  }
  // The tab ends with *Leading to the end* and its foot.
  let bottom = |page: &mut Settings, id: &str| {
    let bounds = page
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    bounds.y + bounds.height
  };
  let (watching, leading) = (
    bottom(&mut page, "settings-orchestrator-watching"),
    bottom(&mut page, "settings-orchestrator-leading"),
  );
  assert!(watching < leading, "{watching} {leading}");
}
