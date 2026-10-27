//! Environment validation, harmless saves, person confirmation and stale-change protection.
use orchester_core::store::SettingsStore;
use orchester_workspace::{tasks::outside::Outside, tools::form::parse_environment};

use super::harness::Settings;

const INPUT: &str = "settings-environment-variables";
#[test]
fn project_settings_environment_validates_each_line_and_saves_harmless_changes_on_leave() {
  let mut page = Settings::new(false);
  page.tab("environment", INPUT);
  for text in ["1NAME=x", "NAME", "NAME=x\nNAME=y"] {
    page.set(INPUT, text);
    page.leave();
    assert!(page.exists("settings-environment-variables-note"));
    assert_eq!(page.db().project_environment().expect("unchanged").iter().count(), 0);
  }
  page.set(INPUT, "RUST_LOG=info\nCI=1");
  page.leave();
  page.saved();
  assert_eq!(
    page.db().project_environment().expect("stored").get("RUST_LOG"),
    Some("info")
  );
  assert_eq!(page.text("settings-variable-count"), "2 variables");
  assert!(!page.exists("settings-environment-question"));
  page.set(INPUT, "");
  page.leave();
  page.saved();
  assert_eq!(page.db().project_environment().expect("cleared").iter().count(), 0);
  page.click("settings-open-secrets");
  assert!(matches!(
    page.window.navigation().route_now(),
    orchester_app::routes::Route::Project(_, orchester_app::routes::ProjectPage::Secrets(_))
  ));
}

#[test]
fn project_settings_environment_widening_is_asked_in_place_and_locks_window_input() {
  let mut page = Settings::new(false);
  page.tab("environment", INPUT);
  page.set(INPUT, "PATH=tools");
  page.leave();
  page.until("question", |p| p.exists("settings-environment-question-confirm"));
  assert!(page.window.link.input_gate().protecting());
  assert_eq!(
    page.db().project_environment().expect("not yet saved").iter().count(),
    0
  );
  assert!(page.text("settings-environment-question").contains("PATH"));
  // The question must live through a future pass before a person's press is taken.
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-environment-question-keep");
  assert!(!page.window.link.input_gate().protecting());
  assert_eq!(page.value(INPUT), "PATH=tools");
  page.set(INPUT, "PATH=tools");
  page.leave();
  page.until("question armed", |p| {
    p.window.tree.tick_futures();
    p.window.pass();
    p.exists("settings-environment-question-confirm")
  });
  page.click("settings-environment-question-confirm");
  page.saved();
  assert_eq!(
    page.db().project_environment().expect("saved").get("PATH"),
    Some("tools")
  );
  assert!(!page.window.link.input_gate().protecting());
}

#[test]
fn project_settings_environment_a_confirmed_change_cannot_overwrite_outside_edits() {
  let mut page = Settings::new(false);
  page.tab("environment", INPUT);
  page.set(INPUT, "PATH=tools");
  page.leave();
  page.until("question", |p| p.exists("settings-environment-question"));
  let outside = parse_environment("CI=1").expect("environment");
  page.db().save_project_environment(&outside).expect("outside save");
  page.outside(Outside::ProjectSettings);
  page.until("outside change is visible", |p| {
    p.exists("settings-environment-outside")
  });
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-environment-question-confirm");
  page.until("failed stale save", |p| p.exists("settings-environment-save-error"));
  assert_eq!(page.db().project_environment().expect("outside kept"), outside);
  assert_eq!(
    page.text("project-settings-save-status"),
    "Not saved · Orchester couldn't write the settings"
  );
  page.click("settings-environment-reload");
  page.saved();
  assert_eq!(page.value(INPUT), "CI=1");
  assert!(!page.exists("settings-environment-save-error"));
  assert!(!page.window.link.input_gate().protecting());
  page.tab("limits", "settings-default-budget");
}

fn failed_save(page: &mut Settings) {
  page.tab("environment", INPUT);
  let sql = rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("fixture DB");
  sql
    .execute_batch(
      "CREATE TRIGGER reject_environment BEFORE INSERT ON project_environment
     BEGIN SELECT RAISE(ABORT, 'regression write refused'); END;",
    )
    .expect("controlled failure");
  page.set(INPUT, "RUST_LOG=debug");
  page.leave();
  page.until("save failure", |p| p.exists("settings-environment-save-error"));
}

#[test]
fn project_settings_environment_failed_save_reverts_without_a_write_and_unlocks_navigation() {
  let mut page = Settings::new(false);
  failed_save(&mut page);
  page.set(INPUT, "");
  page.leave();
  page.saved();
  assert!(!page.exists("settings-environment-save-error"));
  assert!(page.db().project_environment().expect("baseline").is_empty());
  page.tab("limits", "settings-default-budget");
}

#[test]
fn project_settings_environment_reload_recovers_save_failure_only_after_a_fresh_successful_read() {
  let mut page = Settings::new(false);
  failed_save(&mut page);
  let sql = rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("fixture DB");
  sql
    .execute_batch("DROP TRIGGER reject_environment;")
    .expect("allow outside write");
  page
    .db()
    .save_project_environment(&parse_environment("CI=1").expect("outside"))
    .expect("outside save");
  page.outside(Outside::ProjectSettings);
  page.until("Reload offered", |p| p.exists("settings-environment-reload"));
  // Break the stored environment after the notice's read. Reload must not accept that cached read.
  sql
    .execute(
      "INSERT INTO project_environment (name, value) VALUES ('1INVALID', 'x')",
      [],
    )
    .expect("bad fixture row");
  page.click("settings-environment-reload");
  page.until("reload failed", |p| p.exists("settings-environment-read-error"));
  assert_eq!(page.value(INPUT), "RUST_LOG=debug");
  assert!(page.exists("settings-environment-save-error"));
  assert!(page.text("project-settings-save-status").starts_with("Not saved"));
  page.click("settings-tab-general");
  assert!(page.exists(INPUT), "failed Reload cannot unlock section navigation");
  sql
    .execute_batch(
      "DELETE FROM project_environment WHERE name = '1INVALID';
     UPDATE project_environment SET value = '2' WHERE name = 'CI';",
    )
    .expect("repair fixture");
  page.click("settings-environment-reload");
  page.saved();
  assert_eq!(page.value(INPUT), "CI=2", "successful Reload reads current storage");
  assert!(!page.exists("settings-environment-save-error"));
  assert!(!page.exists("settings-environment-read-error"));
  assert!(!page.window.link.input_gate().protecting());
  page.tab("limits", "settings-default-budget");
  page.tab("environment", INPUT);
  assert_eq!(page.value(INPUT), "CI=2");
}
