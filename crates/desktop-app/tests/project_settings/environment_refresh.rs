//! The variables' query read again after a change elsewhere: a failed read says why with Try again over the variables
//! read before, and Try again reads them again; a change made while the editor holds an edit is said with Reload,
//! which takes what is stored at once, and editing then saves on that.
use orchester_core::store::SettingsStore;
use orchester_workspace::{tasks::outside::Outside, tools::form::parse_environment};

use super::harness::Settings;

const INPUT: &str = "settings-environment-variables";
const RELOAD: &str = "settings-environment-reload";

fn fixture_sql(page: &Settings) -> rusqlite::Connection {
  rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("fixture DB")
}

#[test]
fn project_settings_environment_background_read_failure_retries_without_another_outside_event() {
  let mut page = Settings::new(false);
  page.tab("environment", INPUT);
  page.saved();
  assert_eq!(page.value(INPUT), "");
  let sql = fixture_sql(&page);
  sql
    .execute(
      "INSERT INTO project_environment (name, value) VALUES ('1INVALID', 'x')",
      [],
    )
    .expect("bad fixture row");
  page.outside(Outside::ProjectSettings);
  page.until("ordinary refresh failed", |p| {
    p.exists("settings-environment-read-error")
  });
  assert_eq!(page.value(INPUT), "", "untouched baseline stays visible");
  assert!(page.text("project-settings-save-status").starts_with("Not saved"));
  assert_eq!(page.text(RELOAD), "Try again");
  page.click("settings-tab-general");
  assert!(page.exists(INPUT), "failed read keeps navigation locked");

  sql
    .execute_batch("DELETE FROM project_environment; INSERT INTO project_environment VALUES ('CI', '2');")
    .expect("repair storage without another outside event");
  page.window.tree.tick_futures();
  page.window.pass();
  assert!(
    page.exists("settings-environment-read-error"),
    "repair alone cannot retry the read"
  );
  page.click(RELOAD);
  page.until("the variables read again", |p| p.value(INPUT) == "CI=2");
  page.saved();
  assert!(!page.exists("settings-environment-read-error"));
  assert!(!page.exists("settings-environment-outside"));
  page.tab("limits", "settings-default-budget");
}

fn outside_with_draft(page: &mut Settings) {
  page.tab("environment", INPUT);
  page.set(INPUT, "RUST_LOG=debug");
  page
    .db()
    .save_project_environment(&parse_environment("CI=1").expect("outside"))
    .expect("outside save");
  page.outside(Outside::ProjectSettings);
  page.until("outside notice", |p| p.exists(RELOAD));
}

#[test]
fn project_settings_environment_reload_takes_the_stored_variables_and_editing_saves_on_them() {
  let mut page = Settings::new(false);
  outside_with_draft(&mut page);
  assert_eq!(page.value(INPUT), "RUST_LOG=debug", "the edit stays until Reload");
  assert_eq!(page.text("project-settings-save-status"), "Changes not saved");
  page.click(RELOAD);
  page.saved();
  assert_eq!(page.value(INPUT), "CI=1");
  assert!(!page.exists("settings-environment-outside"));
  page.set(INPUT, "CI=2");
  page.leave();
  page.saved();
  assert_eq!(
    page.db().project_environment().expect("new edit saved").get("CI"),
    Some("2")
  );
  assert_eq!(page.value(INPUT), "CI=2", "editing resumes after Reload");
  page.tab("limits", "settings-default-budget");
}

#[test]
fn project_settings_environment_a_change_elsewhere_fills_an_editor_without_an_edit() {
  let mut page = Settings::new(false);
  page.tab("environment", INPUT);
  page.saved();
  page
    .db()
    .save_project_environment(&parse_environment("CI=3").expect("outside"))
    .expect("outside save");
  // `set_project_environment` records the change; the open tab shows it with no notice and no Reload.
  page.outside(Outside::ProjectSettings);
  page.until("the change shown", |p| p.value(INPUT) == "CI=3");
  assert!(!page.exists("settings-environment-outside"));
  assert!(page.exists("settings-variable-CI-row"));
  page.saved();
}
