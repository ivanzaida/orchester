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
  let before = page.db().repository_agent_files().expect("agent files");
  // An adjacent person action also publishes a complete Settings object to AutoMode.
  page.click("settings-agent-files");
  page.until("agent files readback", |p| {
    p.db().repository_agent_files().expect("saved") != before
  });
  page.until("agent files shown from refreshed Settings", |p| {
    p.text("settings-agent-files")
      .starts_with(if before.is_on() { "Off" } else { "On" })
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
  assert!(page.text("settings-reviewer-readback").contains("haiku"));
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
  page.tab("limits", "settings-default-budget");
  page.tab("general", "settings-reviewer-model");
  assert!(page.text("settings-reviewer-readback").contains("sonnet"));
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
