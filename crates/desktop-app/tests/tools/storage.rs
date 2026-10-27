//! A locked database is a wait, not an error (`SsC16`): the library says the database is busy, never claims an empty
//! project, and reads again on its own once the lock is gone, without changing a tool.
use orchester_app::routes::ProjectPage;

use super::harness::Tools;

#[test]
fn tools_database_lock_is_a_wait_that_reads_again_on_its_own() {
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
  // Each read waits out the lock budget first.
  let wait = std::time::Duration::from_secs(30);
  t.window.until_within(wait, "the database is busy", |w| {
    w.texts("tools-load-error")
      .is_some_and(|text| text.join(" ").contains("The project's database is busy"))
  });
  let notice = t.text("tools-load-error");
  assert!(notice.contains("tries again on its own"), "{notice}");
  assert!(!t.exists("tools-load-error-retry"), "it needs no Try again");
  assert!(!t.exists("tools-empty"));
  sqlite.execute_batch("ROLLBACK").expect("release database lock");
  t.window.until_within(wait, "tools list recovers on its own", |w| {
    w.tree.get_element_by_id(&format!("tool-row-{id}")).is_some()
      && w.tree.get_element_by_id("tools-load-error").is_none()
  });
  assert_eq!(t.entry(id), before, "the wait and its reads never change the tool");
}
