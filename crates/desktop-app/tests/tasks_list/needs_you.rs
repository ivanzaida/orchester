//! The Needs you tab counts the tasks it lists; what waits and is no task's (the project's memory records) is
//! named in a notice over its rows, whose *Open Needs you* opens the title bar's panel, which counts it.

use orchester_app::routes::{TaskList, TaskListTab};

use crate::harness::Seeded;

const BODY: &str = "This list shows tasks only. The Needs you panel lists everything that waits for you.";

#[test]
fn tasks_needs_you_with_only_memory_waiting_counts_no_task_and_names_the_memory() {
  let mut seeded = Seeded::tasks("memory-only", TaskList::new(TaskListTab::NeedsYou));
  assert_eq!(seeded.text("tasks-tab-needs_you-label"), "Needs you 0");
  assert!(seeded.keys().is_empty());
  assert_eq!(seeded.text("tasks-no-rows-title"), "No tasks in this tab.");
  assert_eq!(
    seeded.texts("tasks-outside-tasks"),
    ["1 memory record waits for approval", BODY, "Open Needs you"]
  );
  assert_eq!(
    seeded.window.texts("needs-you-pill"),
    Some(vec!["1 needs you".to_owned()]),
    "the pill counts the record"
  );
  assert!(!seeded.shows("needs-you-head"));
  seeded.click("tasks-outside-tasks-open");
  assert_eq!(
    seeded.text("needs-you-count"),
    "1",
    "the panel opens and lists the record"
  );
}

#[test]
fn tasks_needs_you_lists_and_counts_the_waiting_tasks_and_names_the_memory_over_them() {
  let mut seeded = Seeded::tasks("tasks-memory", TaskList::new(TaskListTab::NeedsYou));
  let keys = seeded.keys();
  assert_eq!(keys.len(), 6, "{keys:?}");
  assert_eq!(seeded.text("tasks-tab-needs_you-label"), "Needs you 6");
  assert_eq!(
    seeded.texts("tasks-outside-tasks"),
    ["2 memory records wait for approval", BODY, "Open Needs you"]
  );
  assert_eq!(
    seeded.window.texts("needs-you-pill"),
    Some(vec!["7 need you".to_owned()]),
    "the pill counts the tasks' waits and the records"
  );
  let (notice, table) = (seeded.bounds("tasks-outside-tasks"), seeded.bounds("tasks-table"));
  assert!(notice.y + notice.height <= table.y, "the notice is over the rows");
  seeded.go(TaskList::new(TaskListTab::All));
  assert!(!seeded.shows("tasks-outside-tasks"), "only Needs you names it");
}
