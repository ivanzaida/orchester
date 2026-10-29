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
  assert!(page.reviewer_model().contains("sonnet"));
  page.choose("settings-reviewer-effort", "high");
  page.until("reviewer effort", |p| {
    p.db()
      .auto_mode_settings()
      .expect("settings")
      .reviewer
      .is_some_and(|choice| choice.effort.as_deref() == Some("high"))
  });
  page.saved();
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-model");
  assert!(page.reviewer_model().contains("sonnet"));
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
fn project_settings_review_default_model_edit_with_autopilot_allowed_saves_a_persons_change_at_once() {
  let mut page = Settings::new(true);
  page.click("settings-allow-autopilot");
  page.until("Autopilot allowed", |p| {
    p.db().allow_autopilot().expect("settings").is_on()
  });
  page.saved();
  page.choose("settings-reviewer-model", "sonnet");
  assert_eq!(page.text("project-settings-save-status"), "Saving…");
  assert!(!page.window.link.input_gate().protecting());
  page.until("reviewer saved", |p| {
    p.db()
      .auto_mode_settings()
      .expect("settings")
      .reviewer
      .is_some_and(|choice| choice.model.as_str() == "sonnet")
  });
  page.saved();
}

#[test]
fn project_settings_review_reselecting_a_reviewer_with_autopilot_allowed_saves_each_choice() {
  let mut page = Settings::new(true);
  page.choose("settings-reviewer-connection", "Claude Code");
  page.until("initial reviewer saved", |p| {
    p.db().auto_mode_settings().expect("settings").reviewer.is_some()
  });
  page.saved();
  page.click("settings-allow-autopilot");
  page.until("Autopilot allowed", |p| {
    p.db().allow_autopilot().expect("settings").is_on()
  });
  page.saved();
  // Remount Runs from storage so its reviewer is settled before the edits.
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-model");
  assert!(page.reviewer_model().contains("haiku"));
  for model in ["sonnet", "haiku", "sonnet"] {
    page.choose("settings-reviewer-model", model);
    page.until("the reviewer saved", |p| p.reviewer_model() == model);
    page.saved();
  }
  assert!(!page.exists("settings-auto-question"));
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-model");
  assert!(page.reviewer_model().contains("sonnet"));
}
