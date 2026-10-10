//! Empty limits, validation, saving, status-bar readback and edits made outside the page.
use orchester_core::{
  limits::{AgentSlots, ProjectLimits},
  store::SettingsStore,
  tokens::TokenBudget,
};
use orchester_workspace::tasks::outside::Outside;

use super::harness::Settings;

#[test]
fn project_settings_limits_show_no_limit_and_save_all_fields_without_a_maximum() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  assert_eq!(page.value("settings-default-budget"), "");
  assert!(page.text("settings-default-budget-read-back").contains("No limit"));
  assert!(page.text("settings-time-limit-read-back").contains("No limit"));
  page.set("settings-default-budget", "900000000");
  page.leave();
  page.saved();
  assert_eq!(
    page
      .db()
      .project_limits()
      .expect("limits")
      .default_run_budget()
      .map(TokenBudget::get),
    Some(900_000_000)
  );
  page.set("settings-daily-budget", "");
  page.leave();
  page.saved();
  assert_eq!(page.db().project_limits().expect("limits").daily_budget_limit(), None);
  assert!(page.text("settings-daily-budget-read-back").contains("No limit"));
  page.set("settings-time-limit", "1200");
  page.leave();
  page.saved();
  page.set("settings-agent-slots", "16");
  page.leave();
  page.saved();
  let read = page.db().project_limits().expect("limits");
  assert_eq!(read.default_time_limit().map(|v| v.as_secs()), Some(72000));
  assert_eq!(read.agent_slots().get().get(), 16);
  page.until("the bar's no-limit readback", |p| {
    p.window
      .texts("status-tokens-value")
      .is_some_and(|text| text.join(" ").contains("No limit"))
  });
  page.tab("general", "settings-project-name");
  page.tab("limits", "settings-default-budget");
  assert_eq!(page.value("settings-daily-budget"), "");
  assert_eq!(page.value("settings-agent-slots"), "16");
}

#[test]
fn project_settings_limits_reject_bad_values_in_place_without_saving() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  for (id, text) in [
    ("settings-default-budget", "1.4M"),
    ("settings-daily-budget", "0"),
    ("settings-time-limit", "1.5"),
    ("settings-agent-slots", "17"),
  ] {
    page.set(id, text);
    page.leave();
    assert!(
      page.text(&format!("{id}-note")).contains("Still"),
      "the invalid value keeps its saved readback"
    );
    assert_eq!(page.value(id), text, "the person's edit stays");
  }
  assert_eq!(page.db().project_limits().expect("unchanged"), ProjectLimits::DEFAULT);
  assert_eq!(
    page.text("project-settings-save-status"),
    "Not saved · fix the marked field"
  );
}

#[test]
fn project_settings_limits_preserve_outside_fields_and_reload_unsaved_edits() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  page.set("settings-default-budget", "5000000");
  let outside = ProjectLimits::DEFAULT.with_agent_slots(AgentSlots::new(2).expect("slots"));
  page.db().save_project_limits(&outside).expect("outside save");
  page.outside(Outside::Limits);
  page.until("outside notice", |p| p.exists("settings-changed-outside"));
  assert_eq!(page.value("settings-default-budget"), "5000000");
  page.leave();
  page.saved();
  assert_eq!(page.db().project_limits().expect("merged").agent_slots().get().get(), 2);
  page.set("settings-default-budget", "bad");
  page
    .db()
    .save_project_limits(&ProjectLimits::DEFAULT)
    .expect("another outside save");
  page.outside(Outside::Limits);
  page.until("outside notice", |p| p.exists("settings-reload"));
  page.click("settings-reload");
  assert_eq!(page.value("settings-default-budget"), "");
  assert_eq!(page.value("settings-agent-slots"), "4");
}
