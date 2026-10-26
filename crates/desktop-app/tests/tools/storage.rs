//! The library shows a real database-lock cause and recovers without changing a tool.
use orchester_app::routes::ProjectPage;

use super::harness::Tools;

#[test]
fn tools_database_lock_shows_actionable_cause_and_retry_recovers() {
  let mut t = Tools::new();
  let id = t.seed("Checks", true);
  let before = t.entry(id);
  t.go(ProjectPage::Tools);
  let sqlite = rusqlite::Connection::open(t.repo.join(".orchester/orchester.db")).expect("db");
  // The read gives up once it waited for the lock five seconds instead of a minute (process-wide, for
  // every test of this binary).
  orchester_store::set_lock_budget_for_test(std::time::Duration::from_secs(5));
  // Rollback-journal exclusive locks block readers as well as writers, on both platforms. The window's own reads
  // (the list, the hub's tab counts) come and go meanwhile: the lock is taken between them.
  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
  while let Err(error) = sqlite.execute_batch("PRAGMA journal_mode = DELETE; BEGIN EXCLUSIVE") {
    assert!(std::time::Instant::now() < deadline, "lock fixture database: {error}");
    t.window.tree.tick_futures();
    t.window.pass();
    std::thread::sleep(std::time::Duration::from_millis(10));
  }
  t.window
    .projects()
    .feed(super::support::project_id())
    .outside
    .update(|c| c.tools += 1);
  t.window.until("database-lock cause", |w| {
    w.texts("tools-load-error")
      .is_some_and(|text| text.join(" ").contains("database is locked"))
  });
  let detail = t.text("tools-load-error");
  assert!(
    detail.contains(
      "Reading the tools from the project database failed: database is locked. No tool was changed. The list appears once it can be read."
    ),
    "{detail}"
  );
  assert!(!detail.contains("The log has the details"));
  assert!(!t.exists("tools-empty"));
  sqlite.execute_batch("ROLLBACK").expect("release database lock");
  t.click("tools-load-error-retry");
  t.window.until("tools list recovers", |w| {
    w.tree.get_element_by_id(&format!("tool-row-{id}")).is_some()
      && w.tree.get_element_by_id("tools-load-error").is_none()
  });
  assert_eq!(t.entry(id), before, "the failed read and Retry never change the tool");
}
