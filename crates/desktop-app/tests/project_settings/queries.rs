//! The tabs read through the session's queries (`orchester_app::queries`): a change an MCP domain tool makes shows in
//! the open tab in place, with no reload and no notice, through the change it records; and a tab shown before shows its
//! settings in its first frame, never its skeleton.
use serde_json::json;

use super::{
  domain::{call, consent},
  harness::Settings,
};

#[test]
fn settings_queries_update_project_limits_shows_in_the_open_limits() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-agent-slots");
  page.saved();
  assert_eq!(page.value("settings-agent-slots"), "4");
  call(
    &page,
    "update_project_limits",
    json!({"project": "Kontur", "agent_slots": 7}),
  )
  .expect("limits");
  page.until("the tool's limits shown", |p| p.value("settings-agent-slots") == "7");
  assert!(
    !page.exists("settings-changed-outside"),
    "no notice: nothing was being edited"
  );
  page.saved();
}

#[test]
fn settings_queries_set_allow_autopilot_shows_in_the_open_runs() {
  let mut page = Settings::new(false);
  page.tab("runs", "settings-auto-approve-mode-row");
  page.click("settings-approve-mode-autopilot");
  page.until("Autopilot allowed", |p| {
    p.db().allow_autopilot().expect("saved").is_on()
  });
  page.saved();
  let autopilot = page.text("settings-auto-approve-mode-hint");
  // Automation may always stop allowing Autopilot: the Approve mode moves down to Manual.
  call(
    &page,
    "set_allow_autopilot",
    json!({"project": "Kontur", "allow_autopilot": false}),
  )
  .expect("off");
  page.until("Manual shown", |p| {
    p.text("settings-auto-approve-mode-hint") != autopilot
  });
  assert!(!page.db().allow_autopilot().expect("read").is_on());
}

#[test]
fn settings_queries_set_project_environment_shows_in_the_open_environment() {
  let mut page = Settings::new(false);
  consent(&page);
  page.tab("environment", "settings-environment-variables");
  page.saved();
  call(
    &page,
    "set_project_environment",
    json!({"project": "Kontur", "variables": [{"name": "CI", "value": "1"}]}),
  )
  .expect("environment");
  page.until("the tool's variables shown", |p| {
    p.exists("settings-variable-CI-row") && p.value("settings-environment-variables") == "CI=1"
  });
  assert!(!page.exists("settings-environment-outside"));
}

#[test]
fn settings_queries_a_tab_shown_before_shows_its_settings_in_its_first_frame() {
  let mut page = Settings::new(false);
  page.tab("context", "settings-agent-files");
  page.tab("environment", "settings-environment-edit");
  for (tab, ready) in [
    ("runs", "settings-auto-approve-mode-row"),
    ("orchestrator", "settings-orchestrator-publish-drafts"),
    ("context", "settings-agent-files"),
    ("environment", "settings-environment-edit"),
    ("general", "settings-project-name"),
  ] {
    page.tab(tab, ready);
  }
  for (tab, ready) in [
    ("runs", "settings-auto-approve-mode-row"),
    ("orchestrator", "settings-orchestrator-publish-drafts"),
    ("context", "settings-agent-files"),
    ("environment", "settings-environment-edit"),
  ] {
    // One pass after the press, before any read could end: the tab's settings are there, with no skeleton.
    page.click(&format!("settings-tab-{tab}"));
    assert!(page.exists(ready), "{tab} shows {ready} in its first frame");
    assert!(!page.exists("settings-loading"), "{tab} shows no skeleton");
  }
}
