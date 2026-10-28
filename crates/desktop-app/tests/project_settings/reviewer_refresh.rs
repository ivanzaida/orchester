//! A person's reviewer kept through refreshes, conflict CAS, and explicit failed-save retry.
use lurq::app::events::with_synthetic_input;
use orchester_core::{resource::ModelId, store::SettingsStore};
use orchester_workspace::tasks::outside::Outside;

use super::harness::Settings;

/// Auto mode turned on by the person, saved.
fn turned_on(page: &mut Settings) {
  page.click("settings-auto-new-runs");
  page.until("auto mode on", |p| {
    p.db().auto_mode_settings().expect("settings").new_runs
  });
  page.saved();
}

fn fail_writes(page: &Settings, fail: bool) {
  // Only this disposable project's DB is touched; failure injection has no product hook.
  let db = rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("fixture DB");
  db.execute_batch(if fail {
    "CREATE TRIGGER reject_reviewer BEFORE INSERT ON project_auto_mode
     BEGIN SELECT RAISE(ABORT, 'regression write refused'); END;"
  } else {
    "DROP TRIGGER reject_reviewer;"
  })
  .expect("controlled write failure");
}

#[test]
fn project_settings_unrelated_refresh_keeps_a_persons_saved_reviewer() {
  let mut page = Settings::new(true);
  turned_on(&mut page);
  page.choose("settings-reviewer-model", "sonnet");
  page.until("the reviewer saved", |p| p.reviewer_model() == "sonnet");
  page.saved();
  // An adjacent person edit on Runs also publishes a complete Settings object to AutoMode.
  page.set("settings-agent-slots", "5");
  page.leave();
  page.until("the agents' count saved", |p| {
    p.db().project_limits().expect("limits").agent_slots().get().get() == 5
  });
  page.until("the count shown from refreshed Settings", |p| {
    p.value("settings-agent-slots") == "5"
  });
  page.saved();
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
  assert_eq!(page.reviewer_model(), "sonnet");
}

#[test]
fn project_settings_relevant_refresh_keeps_a_failed_draft_until_reload() {
  let mut page = Settings::new(true);
  turned_on(&mut page);
  fail_writes(&page, true);
  page.choose("settings-reviewer-model", "sonnet");
  page.until("failure", |p| p.exists("settings-auto-retry"));
  fail_writes(&page, false);
  let mut outside = page.db().auto_mode_settings().expect("A");
  let settings = orchester_workspace::project_settings::load_in(&page.repo).expect("settings");
  outside.reviewer =
    orchester_workspace::project_settings::reviewer(&settings, "", "haiku", "").expect("outside choice");
  outside.reviewer.as_mut().expect("explicit outside reviewer").model = ModelId::new("haiku").expect("model");
  page.db().save_auto_mode_settings(&outside).expect("outside change");
  page.outside(Outside::AutoMode);
  page.until("conflict", |p| p.exists("settings-auto-reload"));
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
  page.click("settings-auto-retry");
  assert_eq!(
    page.db().auto_mode_settings().expect("outside retained"),
    outside,
    "a conflict requires Reload"
  );
  page.click("settings-auto-reload");
  page.saved();
  assert_eq!(page.db().auto_mode_settings().expect("outside retained"), outside);
  assert!(page.text("settings-reviewer-model").contains("haiku"));
}

#[test]
fn project_settings_failed_reviewer_save_retries_same_choice_only_on_explicit_action() {
  let mut page = Settings::new(true);
  let original = page.db().auto_mode_settings().expect("A");
  fail_writes(&page, true);
  page.choose("settings-reviewer-model", "sonnet");
  page.until("failure", |p| p.exists("settings-auto-retry"));
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
  assert_eq!(page.db().auto_mode_settings().expect("A retained"), original);
  fail_writes(&page, false);
  for _ in 0..30 {
    page.window.tree.tick_futures();
    page.window.pass();
  }
  assert!(page.text("project-settings-save-status").contains("couldn't write"));
  assert_eq!(page.db().auto_mode_settings().expect("no automatic retries"), original);
  page.click("settings-auto-retry");
  page.saved();
  assert_eq!(page.reviewer_model(), "sonnet");
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-model");
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
}

/// Retrying a widening makes it again: automation's press is put back, a person's saves.
#[test]
fn project_settings_failed_widening_is_retried_only_by_a_person() {
  let mut page = Settings::new(true);
  turned_on(&mut page);
  let original = page.db().auto_mode_settings().expect("A");
  fail_writes(&page, true);
  page.choose("settings-reviewer-model", "sonnet");
  page.until("failure", |p| p.exists("settings-auto-retry"));
  fail_writes(&page, false);
  with_synthetic_input(|| page.click("settings-auto-retry"));
  page.until("the retry put back", |p| p.exists("settings-auto-person-only"));
  for _ in 0..10 {
    page.window.tree.tick_futures();
    page.window.pass();
  }
  assert_eq!(page.db().auto_mode_settings().expect("not retried"), original);
  page.choose("settings-reviewer-model", "sonnet");
  page.until("a person's reviewer saved", |p| p.reviewer_model() == "sonnet");
  page.saved();
}

#[test]
fn project_settings_failed_reviewer_reverting_to_baseline_abandons_retry() {
  for auto in [false, true] {
    let mut page = Settings::new(true);
    page.choose("settings-reviewer-connection", "Claude Code");
    page.saved();
    page.choose("settings-reviewer-model", "haiku");
    page.saved();
    if auto {
      turned_on(&mut page);
    }
    let original = page.db().auto_mode_settings().expect("A");
    fail_writes(&page, true);
    page.choose("settings-reviewer-model", "sonnet");
    page.until("B failed", |p| p.exists("settings-auto-retry"));
    assert!(page.text("settings-reviewer-model").contains("sonnet"));
    // Leaving the failure trigger installed proves abandonment does not need another write.
    page.choose("settings-reviewer-model", "haiku");
    page.saved();
    assert!(page.text("settings-reviewer-model").contains("haiku"));
    assert!(!page.exists("settings-auto-retry"));
    assert_eq!(page.db().auto_mode_settings().expect("A retained"), original);
    page.tab("general", "settings-project-name");
    page.tab("runs", "settings-reviewer-model");
    assert!(page.text("settings-reviewer-model").contains("haiku"));
    assert_eq!(page.db().auto_mode_settings().expect("no abandoned B"), original);
  }
}

#[test]
fn project_settings_failed_reviewer_new_draft_saves_only_the_visible_choice() {
  let mut page = Settings::new(true);
  turned_on(&mut page);
  let original = page.db().auto_mode_settings().expect("A");
  fail_writes(&page, true);
  page.choose("settings-reviewer-model", "sonnet");
  page.until("B failed", |p| p.exists("settings-auto-retry"));
  // C differs from B in effort, and is saved on its own.
  page.choose("settings-reviewer-effort", "high");
  page.until("C failed", |p| {
    p.exists("settings-auto-retry") && p.text("project-settings-save-status").contains("couldn't write")
  });
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
  assert!(page.text("settings-reviewer-effort").contains("high"));
  assert_eq!(page.db().auto_mode_settings().expect("C not stored"), original);
  fail_writes(&page, false);
  page.click("settings-auto-retry");
  page.saved();
  let saved = page.db().auto_mode_settings().expect("C").reviewer.expect("reviewer");
  assert_eq!(saved.model.as_str(), "sonnet");
  assert_eq!(saved.effort.as_deref(), Some("high"));
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-model");
  assert!(page.text("settings-reviewer-effort").contains("high"));
}

#[test]
fn project_settings_failed_auto_switch_keeps_explicit_retry() {
  let mut page = Settings::new(true);
  fail_writes(&page, true);
  page.click("settings-auto-new-runs");
  page.until("enable failed", |p| p.exists("settings-auto-retry"));
  assert!(!page.db().auto_mode_settings().expect("still Off").new_runs);
  for _ in 0..30 {
    page.window.tree.tick_futures();
    page.window.pass();
  }
  assert!(
    page.exists("settings-auto-retry"),
    "a switch failure is not reviewer abandonment"
  );
  fail_writes(&page, false);
  page.click("settings-auto-retry");
  page.saved();
  assert!(page.db().auto_mode_settings().expect("enabled").new_runs);
}
