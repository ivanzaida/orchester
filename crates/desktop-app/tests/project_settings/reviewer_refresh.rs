//! Draft/question preservation, conflict CAS and explicit failed-save retry.
use orchester_core::{resource::ModelId, store::SettingsStore};
use orchester_workspace::tasks::outside::Outside;

use super::harness::Settings;

// A headless tree click still respects the card's first-pass guard. Wait for it to arm.
fn start_save(page: &mut Settings) {
  page.until("save starts after the headless card is armed", |p| {
    p.click("settings-auto-question-confirm");
    p.text("project-settings-save-status") == "Saving…"
  });
}

fn question(page: &mut Settings) {
  page.click("settings-auto-new-runs");
  page.until("enable question", |p| p.exists("settings-auto-question-confirm"));
  page.window.tree.tick_futures();
  page.window.pass();
  start_save(page);
  page.saved();
  page.choose("settings-reviewer-model", "sonnet");
  page.until("reviewer question", |p| p.exists("settings-auto-question-confirm"));
}

#[test]
fn project_settings_unrelated_refresh_preserves_reviewer_question_and_input_gate() {
  let mut page = Settings::new(true);
  question(&mut page);
  let original = page.db().auto_mode_settings().expect("A");
  // An adjacent person edit on Runs also publishes a complete Settings object to AutoMode.
  page.set("settings-agent-slots", "5");
  page.leave();
  page.until("the agents' count saved", |p| {
    p.db().project_limits().expect("limits").agent_slots().get().get() == 5
  });
  page.until("the count shown from refreshed Settings", |p| {
    p.value("settings-agent-slots") == "5"
  });
  assert!(page.exists("settings-auto-question-confirm"));
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
  assert!(page.window.link.input_gate().protecting());
  assert_eq!(page.text("project-settings-save-status"), "Changes not saved");
  assert_eq!(page.db().auto_mode_settings().expect("A retained"), original);
  start_save(&mut page);
  page.saved();
  assert_eq!(
    page
      .db()
      .auto_mode_settings()
      .expect("B")
      .reviewer
      .expect("chosen")
      .model
      .as_str(),
    "sonnet"
  );
}

#[test]
fn project_settings_relevant_refresh_keeps_draft_and_old_confirm_cannot_overwrite() {
  let mut page = Settings::new(true);
  question(&mut page);
  let mut outside = page.db().auto_mode_settings().expect("A");
  let settings = orchester_workspace::project_settings::load_in(&page.repo).expect("settings");
  outside.reviewer =
    orchester_workspace::project_settings::reviewer(&settings, "", "haiku", "").expect("outside choice");
  outside.reviewer.as_mut().expect("explicit outside reviewer").model = ModelId::new("haiku").expect("model");
  page.db().save_auto_mode_settings(&outside).expect("outside change");
  page.outside(Outside::AutoMode);
  page.until("conflict", |p| p.exists("settings-auto-reload"));
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
  assert!(page.exists("settings-auto-question-confirm"));
  assert!(page.window.link.input_gate().protecting());
  start_save(&mut page);
  page.until("stale failure", |p| p.exists("settings-auto-retry"));
  assert_eq!(page.db().auto_mode_settings().expect("outside retained"), outside);
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
  page.click("settings-auto-retry");
  assert!(
    !page.exists("settings-auto-question-confirm"),
    "conflict requires Reload"
  );
  page.click("settings-auto-reload");
  page.saved();
  assert_eq!(page.db().auto_mode_settings().expect("outside retained"), outside);
  assert!(page.text("settings-reviewer-model").contains("haiku"));
  assert!(!page.window.link.input_gate().protecting());
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
  assert_eq!(
    page
      .db()
      .auto_mode_settings()
      .expect("B")
      .reviewer
      .expect("chosen")
      .model
      .as_str(),
    "sonnet"
  );
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-model");
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
}

#[test]
fn project_settings_failed_widening_retry_requires_a_fresh_person_question() {
  let mut page = Settings::new(true);
  question(&mut page);
  let original = page.db().auto_mode_settings().expect("A");
  fail_writes(&page, true);
  start_save(&mut page);
  page.until("failure", |p| p.exists("settings-auto-retry"));
  assert!(!page.window.link.input_gate().protecting());
  fail_writes(&page, false);
  page.click("settings-auto-retry");
  assert!(page.exists("settings-auto-question-confirm"));
  assert!(page.window.link.input_gate().protecting());
  assert_eq!(page.db().auto_mode_settings().expect("not retried yet"), original);
  start_save(&mut page);
  page.saved();
  assert_eq!(
    page
      .db()
      .auto_mode_settings()
      .expect("B")
      .reviewer
      .expect("chosen")
      .model
      .as_str(),
    "sonnet"
  );
}

#[test]
fn project_settings_failed_reviewer_reverting_to_baseline_abandons_retry_and_question() {
  for widening in [false, true] {
    let mut page = Settings::new(true);
    page.choose("settings-reviewer-connection", "Claude Code");
    page.saved();
    page.choose("settings-reviewer-model", "haiku");
    page.saved();
    if widening {
      question(&mut page);
    }
    let original = page.db().auto_mode_settings().expect("A");
    fail_writes(&page, true);
    if widening {
      start_save(&mut page);
    } else {
      page.choose("settings-reviewer-model", "sonnet");
    }
    page.until("B failed", |p| p.exists("settings-auto-retry"));
    assert!(page.text("settings-reviewer-model").contains("sonnet"));
    // Leaving the failure trigger installed proves abandonment does not need another write.
    page.choose("settings-reviewer-model", "haiku");
    page.saved();
    assert!(page.text("settings-reviewer-model").contains("haiku"));
    assert!(!page.exists("settings-auto-retry"));
    assert!(!page.exists("settings-auto-question"));
    assert!(!page.window.link.input_gate().protecting());
    assert_eq!(page.db().auto_mode_settings().expect("A retained"), original);
    page.tab("general", "settings-project-name");
    page.tab("runs", "settings-reviewer-model");
    assert!(page.text("settings-reviewer-model").contains("haiku"));
    assert_eq!(page.db().auto_mode_settings().expect("no abandoned B"), original);
  }
}

#[test]
fn project_settings_failed_reviewer_new_draft_confirms_and_saves_only_visible_choice() {
  let mut page = Settings::new(true);
  question(&mut page);
  let original = page.db().auto_mode_settings().expect("A");
  fail_writes(&page, true);
  start_save(&mut page);
  page.until("B failed", |p| p.exists("settings-auto-retry"));
  // C differs from B in effort. It gets its own question and cannot reuse B's confirmation.
  page.choose("settings-reviewer-effort", "high");
  page.until("C question", |p| p.exists("settings-auto-question-confirm"));
  assert!(page.text("settings-reviewer-model").contains("sonnet"));
  assert!(page.text("settings-reviewer-effort").contains("high"));
  assert!(page.window.link.input_gate().protecting());
  assert_eq!(page.db().auto_mode_settings().expect("C not confirmed"), original);
  start_save(&mut page);
  page.until("C failed", |p| p.exists("settings-auto-retry"));
  fail_writes(&page, false);
  page.click("settings-auto-retry");
  assert!(page.exists("settings-auto-question-confirm"));
  assert!(page.text("settings-reviewer-effort").contains("high"));
  assert_eq!(
    page.db().auto_mode_settings().expect("retry still needs confirmation"),
    original
  );
  start_save(&mut page);
  page.saved();
  let saved = page.db().auto_mode_settings().expect("C").reviewer.expect("reviewer");
  assert_eq!(saved.model.as_str(), "sonnet");
  assert_eq!(saved.effort.as_deref(), Some("high"));
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-model");
  assert!(page.text("settings-reviewer-effort").contains("high"));
}

#[test]
fn project_settings_failed_auto_switch_keeps_explicit_retry_and_fresh_confirmation() {
  let mut page = Settings::new(true);
  let original = page.db().auto_mode_settings().expect("Off");
  fail_writes(&page, true);
  page.click("settings-auto-new-runs");
  page.until("enable question", |p| p.exists("settings-auto-question-confirm"));
  start_save(&mut page);
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
  assert!(page.exists("settings-auto-question-confirm"));
  assert!(page.window.link.input_gate().protecting());
  assert_eq!(page.db().auto_mode_settings().expect("not confirmed"), original);
  start_save(&mut page);
  page.saved();
  assert!(page.db().auto_mode_settings().expect("enabled").new_runs);
}
