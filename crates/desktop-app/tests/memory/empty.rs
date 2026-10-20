//! The Memory screens with no record (`tasks` scenario): the task's tab says what agents keep and which step works
//! (`Empty · no records yet` `e8oWP4`), with no toolbar, as its own *New record* stands in the state; the project's
//! page shows its empty state (`Empty project memory` `T10K7`) without a queue; and a task's Artifacts tab says runs
//! keep no artifacts yet.

use super::harness::Page;

#[test]
fn memory_empty_task_tab_says_what_agents_keep() {
  let mut page = Page::seeded("tasks");
  page.task_memory("KON-12");
  let empty = page.text("memory-empty");
  assert!(empty.starts_with("No memory for this task yet"), "{empty}");
  assert!(empty.contains("03 Implement is working"), "{empty}");
  assert!(
    !page.has("memory-toolbar"),
    "no toolbar while there is nothing to search or count"
  );
  assert!(page.has("memory-empty-new"));
  assert_eq!(page.text("task-tab-memory"), "Memory");
  assert!(page.text("memory-next").contains("Nothing to give yet"));
  page.window.press("memory-empty-new");
  page.until("the new record's editor", |window| {
    window.texts("memory-editor-new").is_some()
  });
}

#[test]
fn memory_empty_project_page_shows_its_empty_state() {
  let mut page = Page::seeded("tasks");
  page.project_memory();
  let empty = page.text("memory-empty");
  assert!(empty.starts_with("No project memory yet"), "{empty}");
  assert!(
    empty.contains("Project memory saves after approval, task memory automatically"),
    "{empty}"
  );
  assert!(!page.has("memory-queue"));
  assert!(page.has("memory-panel"));
}

#[test]
fn memory_artifacts_tab_says_runs_keep_none_yet() {
  let mut page = Page::seeded("memory");
  page.task_artifacts("KON-12");
  assert_eq!(page.text("artifacts-count"), "0 artifacts");
  let run = page.text("artifacts-run");
  assert!(run.starts_with("Run ") && run.ends_with(" · latest"), "{run}");
  assert!(page.text("artifacts-empty-title").starts_with("No artifacts in Run "));
  assert!(
    page
      .text("artifacts-empty-body")
      .contains("Runs don't keep artifacts yet")
  );
  page.window.press("artifacts-run");
  page.until("the run menu", |window| window.texts("artifacts-run-menu").is_some());
  let runs = page.text("artifacts-run-menu");
  assert_eq!(runs.matches("Run ").count(), 3, "KON-12's three runs: {runs}");
}
