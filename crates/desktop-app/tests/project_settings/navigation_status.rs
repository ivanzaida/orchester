//! Tab selection never supplies evidence that an unfinished edit has been saved.
use orchester_core::store::SettingsStore;

use super::harness::Settings;

#[test]
fn project_settings_tab_reselection_and_navigation_keep_invalid_limits_truthful() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  let original = page.db().project_limits().expect("limits");
  for (id, invalid, valid) in [
    ("settings-default-budget", "1.5", ""),
    ("settings-agent-slots", "17", "4"),
  ] {
    page.set(id, invalid);
    page.leave();
    page.click("settings-tab-limits");
    assert_eq!(page.value(id), invalid);
    assert_eq!(
      page.text("project-settings-save-status"),
      "Not saved · fix the marked field"
    );
    page.click("settings-tab-general");
    assert!(page.exists(id), "unfinished section stays visible");
    assert_eq!(page.value(id), invalid);
    assert_eq!(page.db().project_limits().expect("unchanged"), original);
    page.set(id, valid);
    page.leave();
    page.saved();
  }
  page.tab("general", "settings-project-name");
}

#[test]
fn project_settings_tab_reselection_keeps_invalid_and_dirty_environment_truthful() {
  let mut page = Settings::new(false);
  page.tab("environment", "settings-environment-variables");
  page.set("settings-environment-variables", "1NAME=value");
  page.leave();
  page.click("settings-tab-environment");
  assert_eq!(
    page.text("project-settings-save-status"),
    "Not saved · fix the marked field"
  );
  page.click("settings-tab-limits");
  assert_eq!(page.value("settings-environment-variables"), "1NAME=value");
  assert!(page.db().project_environment().expect("not persisted").is_empty());
  page.set("settings-environment-variables", "RUST_LOG=debug");
  assert_eq!(page.text("project-settings-save-status"), "Changes not saved");
  page.click("settings-tab-environment");
  assert_eq!(page.text("project-settings-save-status"), "Changes not saved");
  page.leave();
  page.saved();
  page.tab("limits", "settings-default-budget");
  page.tab("environment", "settings-environment-variables");
  assert_eq!(page.value("settings-environment-variables"), "RUST_LOG=debug");
}
