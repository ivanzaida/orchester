//! Save and refresh regressions returned by the project-settings review.
use orchester_core::{store::SettingsStore, tokens::TokenBudget};

use super::harness::Settings;

#[test]
fn project_settings_review_default_model_selection_persists_the_effective_reviewer() {
  let mut page = Settings::new(true);
  page.choose("settings-reviewer-model", "sonnet");
  page.until("explicit reviewer", |p| {
    p.db()
      .auto_mode_settings()
      .expect("settings")
      .reviewer
      .is_some_and(|choice| choice.model.as_str() == "sonnet")
  });
  page.saved();
  assert!(page.text("settings-reviewer-readback").contains("sonnet"));
  page.choose("settings-reviewer-effort", "high");
  page.until("reviewer effort", |p| {
    p.db()
      .auto_mode_settings()
      .expect("settings")
      .reviewer
      .is_some_and(|choice| choice.effort.as_deref() == Some("high"))
  });
  page.saved();
  page.tab("limits", "settings-default-budget");
  page.tab("general", "settings-reviewer-model");
  assert!(page.text("settings-reviewer-readback").contains("sonnet"));
}

#[test]
fn project_settings_review_second_limits_blur_during_save_is_committed() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  page.set("settings-default-budget", "5000000");
  page.leave();
  assert_eq!(page.text("project-settings-save-status"), "Saving…");
  // No future tick between these blurs: the first save is still pending in the window.
  page.set("settings-time-limit", "1200");
  page.leave();
  page.saved();
  let limits = page.db().project_limits().expect("limits");
  assert_eq!(limits.default_run_budget().map(TokenBudget::get), Some(5_000_000));
  assert_eq!(limits.default_time_limit().map(|v| v.as_secs()), Some(72_000));
  page.tab("general", "settings-project-name");
  page.tab("limits", "settings-time-limit");
  assert_eq!(page.value("settings-time-limit"), "1200");
}

#[test]
fn project_settings_review_limits_revert_during_save_is_committed() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  page.set("settings-default-budget", "5000000");
  page.leave();
  page.set("settings-default-budget", "");
  page.leave();
  page.saved();
  assert_eq!(page.db().project_limits().expect("limits").default_run_budget(), None);
}

#[test]
fn project_settings_review_default_model_edit_in_auto_mode_requires_person_confirmation() {
  let mut page = Settings::new(true);
  page.click("settings-auto-new-runs");
  page.until("enable question", |p| p.exists("settings-auto-question-confirm"));
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-auto-question-confirm");
  page.saved();
  page.choose("settings-reviewer-model", "sonnet");
  page.until("reviewer question", |p| p.exists("settings-auto-question-confirm"));
  assert!(page.window.link.input_gate().protecting());
  assert_eq!(page.db().auto_mode_settings().expect("settings").reviewer, None);
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-auto-question-confirm");
  page.until("reviewer saved", |p| {
    p.db()
      .auto_mode_settings()
      .expect("settings")
      .reviewer
      .is_some_and(|choice| choice.model.as_str() == "sonnet")
  });
  page.saved();
  assert!(!page.window.link.input_gate().protecting());
}

#[test]
fn project_settings_review_reselecting_a_reviewer_after_keep_asks_again_and_saves() {
  let mut page = Settings::new(true);
  page.choose("settings-reviewer-connection", "Claude Code");
  page.until("initial reviewer saved", |p| {
    p.db().auto_mode_settings().expect("settings").reviewer.is_some()
  });
  page.saved();
  page.click("settings-auto-new-runs");
  page.until("enable question", |p| p.exists("settings-auto-question-confirm"));
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-auto-question-confirm");
  page.saved();
  // Remount General from storage so its readback is settled before the cancelled edit.
  page.tab("limits", "settings-default-budget");
  page.tab("general", "settings-reviewer-model");
  let original = page.db().auto_mode_settings().expect("settings");
  assert!(original.new_runs);
  assert!(
    original
      .reviewer
      .as_ref()
      .expect("reviewer")
      .model
      .as_str()
      .contains("haiku")
  );
  page.choose("settings-reviewer-model", "sonnet");
  page.until("first reviewer question", |p| {
    p.exists("settings-auto-question-confirm")
  });
  assert_eq!(page.text("project-settings-save-status"), "Changes not saved");
  assert!(page.window.link.input_gate().protecting());
  assert_eq!(page.db().auto_mode_settings().expect("unchanged"), original);
  page.click("settings-auto-question-keep");
  assert!(!page.exists("settings-auto-question"));
  assert!(!page.window.link.input_gate().protecting());
  assert_eq!(page.text("project-settings-save-status"), "All changes saved");
  assert!(page.text("settings-reviewer-readback").contains("haiku"));
  assert_eq!(page.db().auto_mode_settings().expect("kept"), original);

  page.choose("settings-reviewer-model", "sonnet");
  page.until("reselected reviewer question", |p| {
    p.exists("settings-auto-question-confirm")
  });
  assert_eq!(page.text("project-settings-save-status"), "Changes not saved");
  assert_eq!(page.value("settings-reviewer-model"), "sonnet");
  assert!(page.window.link.input_gate().protecting());
  assert_eq!(page.db().auto_mode_settings().expect("not confirmed"), original);
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-auto-question-confirm");
  assert_eq!(page.text("project-settings-save-status"), "Saving…");
  page.until("reselected reviewer saved", |p| {
    p.db()
      .auto_mode_settings()
      .expect("settings")
      .reviewer
      .is_some_and(|choice| choice.model.as_str() == "sonnet")
  });
  page.saved();
  assert!(!page.exists("settings-auto-question"));
  assert!(!page.window.link.input_gate().protecting());
  assert!(page.text("settings-reviewer-readback").contains("sonnet"));
  page.tab("limits", "settings-default-budget");
  page.tab("general", "settings-reviewer-model");
  assert!(page.text("settings-reviewer-readback").contains("sonnet"));
}
