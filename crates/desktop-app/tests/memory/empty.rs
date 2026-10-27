//! The Memory screens with no record (`tasks` scenario): the task's memory says what agents keep and which step works
//! (`Empty · no records yet` `e8oWP4`), with no toolbar, as its own *New record* stands in the state, and its Summary's
//! link says it has no notes; each tab of the project's page says it has no records (`ejSHj`); and a task's Summary
//! says in its Output that runs keep no artifacts yet.

use orchester_app::routes::MemoryTab;

use super::harness::Page;

#[test]
fn memory_empty_task_memory_says_what_agents_keep() {
  let mut page = Page::seeded("tasks");
  page.task_summary("KON-12");
  // The link counts the task's notes once they are read.
  page.until("the count of the task's notes", |window| {
    window
      .texts("task-memory-link-label")
      .is_some_and(|texts| texts.join(" ") == "No memory notes")
  });
  page.window.press("task-memory-link");
  page.until("the task's memory", |window| window.texts("memory-list").is_some());
  let empty = page.text("memory-empty");
  assert!(empty.starts_with("No memory for this task yet"), "{empty}");
  assert!(empty.contains("03 Implement is working"), "{empty}");
  assert!(
    !page.has("memory-toolbar"),
    "no toolbar while there is nothing to search or count"
  );
  assert!(page.has("memory-empty-new"));
  assert!(page.text("memory-next").contains("Nothing to give yet"));
  page.window.press("memory-empty-new");
  page.until("the new record's editor", |window| {
    window.texts("memory-editor-new").is_some()
  });
}

#[test]
fn memory_empty_project_tabs_say_they_have_no_records() {
  let mut page = Page::seeded("tasks");
  for (tab, title) in [
    (MemoryTab::Active, "No active records"),
    (MemoryTab::Deprecated, "No deprecated records"),
    (MemoryTab::Superseded, "No superseded versions"),
    (MemoryTab::Rejected, "No rejected records"),
  ] {
    page.memory_tab(tab);
    assert_eq!(page.text("memory-empty-title"), title, "{tab:?}");
    assert!(!page.has("memory-search"), "{tab:?} hides its search");
  }
  let rejected = page.text("memory-empty");
  assert!(
    rejected.ends_with("So are proposals an agent replaced with a newer one before anyone decided."),
    "{rejected}"
  );
  assert!(!page.has("memory-panel"), "no side panel");
}

#[test]
fn memory_summary_output_says_runs_keep_no_artifacts_yet() {
  let mut page = Page::seeded("tasks");
  page.task_summary("KON-12");
  assert_eq!(page.text("task-output-head"), "Output 0");
  let none = page.text("task-output-none");
  assert!(none.contains("runs keep no other artifacts yet"), "{none}");
}
