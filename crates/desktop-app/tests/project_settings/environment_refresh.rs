//! Fresh read recovery and mutually exclusive Environment Reload/save/input.
use orchester_core::store::SettingsStore;
use orchester_workspace::{tasks::outside::Outside, tools::form::parse_environment};

use super::harness::Settings;

const INPUT: &str = "settings-environment-variables";
const RELOAD: &str = "settings-environment-reload";

fn fixture_sql(page: &Settings) -> rusqlite::Connection {
  rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("fixture DB")
}

#[test]
fn project_settings_environment_background_read_failure_retries_fresh_without_another_outside_event() {
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
  assert_eq!(page.text("project-settings-save-status"), "Saving…");
  assert_eq!(
    page.value(INPUT),
    "",
    "only a settled fresh read can replace the baseline"
  );
  page.click("settings-tab-general");
  assert!(page.exists(INPUT), "pending retry cannot unlock navigation");
  page.saved();
  assert_eq!(page.value(INPUT), "CI=2");
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
fn project_settings_environment_pending_reload_blocks_typing_blur_save_and_repeated_reload() {
  let mut page = Settings::new(false);
  outside_with_draft(&mut page);
  let sql = fixture_sql(&page);
  sql
    .execute_batch(
      "CREATE TABLE environment_writes (name TEXT);
     CREATE TRIGGER audit_environment AFTER INSERT ON project_environment
     BEGIN INSERT INTO environment_writes VALUES (NEW.name); END;",
    )
    .expect("audit writes");
  page.click(RELOAD);
  // No future ticks until all attempts below have run: Reload remains pending in the window.
  assert_eq!(page.text("project-settings-save-status"), "Saving…");
  assert!(
    page.exists("settings-environment-reload-icon"),
    "pending Reload has its loader"
  );
  page.window.tree.get_element_by_id_mut(INPUT).expect("field").focus();
  page.window.pass();
  page
    .window
    .tree
    .key_down("x".into(), "KeyX".into(), false, false, false);
  page.window.pass();
  assert_eq!(page.value(INPUT), "RUST_LOG=debug", "pending read takes no edit");
  page.leave();
  page.click(RELOAD);
  assert_eq!(page.value(INPUT), "RUST_LOG=debug");
  assert!(!page.exists("settings-environment-question"));
  assert_eq!(page.text("project-settings-save-status"), "Saving…");
  page.click("settings-tab-general");
  assert!(page.exists(INPUT), "pending Reload cannot leave the section");
  page.saved();
  assert_eq!(page.value(INPUT), "CI=1");
  assert!(!page.exists("settings-environment-save-error"));
  assert_eq!(
    sql
      .query_row("SELECT COUNT(*) FROM environment_writes", [], |row| row
        .get::<_, i64>(0))
      .expect("write count"),
    0,
    "blur during Reload must not start a save"
  );

  page.set(INPUT, "CI=2");
  page.leave();
  page.saved();
  assert_eq!(
    page.db().project_environment().expect("new edit saved").get("CI"),
    Some("2")
  );
  assert_eq!(page.value(INPUT), "CI=2", "editing resumes after Reload settles");
  page.tab("limits", "settings-default-budget");
}

#[test]
fn project_settings_environment_pending_save_blocks_reload_and_preserves_its_cas_failure() {
  let mut page = Settings::new(false);
  outside_with_draft(&mut page);
  let sql = fixture_sql(&page);
  sql
    .execute(
      "INSERT INTO project_environment (name, value) VALUES ('1INVALID', 'x')",
      [],
    )
    .expect("bad fixture row");
  page.outside(Outside::ProjectSettings);
  page.until("refresh failed", |p| p.exists("settings-environment-read-error"));
  sql
    .execute("DELETE FROM project_environment WHERE name = '1INVALID'", [])
    .expect("repair without another outside event");
  page.leave();
  assert!(!page.exists("settings-environment-save-error"), "save has not settled");
  // The stale save is still pending in the UI even if its worker has already rejected it.
  page.click(RELOAD);
  assert_eq!(page.value(INPUT), "RUST_LOG=debug");
  assert!(
    !page.exists("settings-environment-reload-icon"),
    "Reload cannot start during a save"
  );
  page.until("CAS save failure", |p| p.exists("settings-environment-save-error"));
  assert!(page.text("project-settings-save-status").starts_with("Not saved"));
  assert_eq!(
    page.value(INPUT),
    "RUST_LOG=debug",
    "Reload did not hide the failed save"
  );
  assert_eq!(
    page.db().project_environment().expect("outside kept").get("CI"),
    Some("1")
  );
  page.click(RELOAD);
  assert!(
    page.exists("settings-environment-save-error"),
    "pending read retains the save error"
  );
  assert_eq!(page.text("project-settings-save-status"), "Saving…");
  page.saved();
  assert_eq!(page.value(INPUT), "CI=1");
  assert!(!page.exists("settings-environment-save-error"));
  page.tab("limits", "settings-default-budget");
}
