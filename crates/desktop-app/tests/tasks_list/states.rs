//! What the list shows instead of rows: a project without tasks, the tasks being read, reading them
//! failing, and a search that hides every row. Loading and a failure never look like an empty list.

use std::{thread, time::Duration};

use orchester_app::routes::{TaskList, TaskListTab};

use crate::harness::Seeded;

/// Whether the list shows anything that says it has no tasks.
fn looks_empty(seeded: &mut Seeded) -> bool {
  seeded.shows("tasks-empty") || seeded.shows("tasks-no-rows") || seeded.shows("tasks-footer")
}

#[test]
fn tasks_a_project_without_tasks_says_so_with_new_task() {
  let mut seeded = Seeded::new("empty");
  seeded.go(TaskList::default());
  seeded.until("the empty project", |seeded| seeded.shows("tasks-empty"));
  assert_eq!(
    seeded.texts("tasks-empty"),
    [
      "No tasks yet",
      "Describe what agents should build as a task. Drafts can be saved before every detail is known.",
      "New task"
    ]
  );
  assert!(!seeded.shows("tasks-tabs"), "no tab to choose");
  assert!(!seeded.shows("tasks-query"));
  seeded.click("tasks-empty-new-task");
  assert_eq!(
    seeded.window.navigation().route_now(),
    orchester_app::routes::Route::Project(seeded.project, orchester_app::routes::ProjectPage::NewTask)
  );
}

#[test]
fn tasks_loading_shows_the_reader_and_dashes_never_an_empty_list() {
  let mut seeded = Seeded::reading("tasks", true);
  let hold = seeded
    .window
    .launch
    .services()
    .expect("the session started")
    .hold
    .clone();
  seeded.go(TaskList::default());
  seeded.until("the list", |seeded| seeded.shows("screen-tasks"));
  for _ in 0..20 {
    seeded.window.tree.tick_futures();
    seeded.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(seeded.text("tasks-loading"), "Reading Kontur's tasks…");
  assert_eq!(seeded.text("tasks-tab-all-label"), "All —");
  assert!(seeded.shows("tasks-table-header"));
  assert!(!looks_empty(&mut seeded));
  hold.set(false);
  seeded.until("the rows", |seeded| seeded.shows("tasks-footer"));
  assert!(!seeded.shows("tasks-loading"));
}

#[test]
fn tasks_a_failed_reading_says_why_with_try_again_never_an_empty_list() {
  let mut seeded = Seeded::new("tasks-unreadable");
  seeded.go(TaskList::default());
  seeded.until("the failure", |seeded| seeded.shows("tasks-load-error"));
  let said = seeded.texts("tasks-load-error");
  assert_eq!(said.first().map(String::as_str), Some("Tasks cannot be read"));
  assert!(
    said[1].starts_with("Reading the task list from the project database failed.")
      && said[1].ends_with("No task was changed. The list appears once it can be read."),
    "{said:?}"
  );
  assert_eq!(said.last().map(String::as_str), Some("Try again"));
  assert_eq!(seeded.text("tasks-tab-all-label"), "All —");
  assert!(!looks_empty(&mut seeded));
  let repaired = rusqlite::Connection::open(seeded.repo.join(".orchester").join("orchester.db"))
    .expect("the project database opens")
    .execute("UPDATE tasks SET status = 'draft'", [])
    .expect("the task is repaired");
  assert_eq!(repaired, 1);
  seeded.click("tasks-load-error-retry");
  seeded.until("the task read again", |seeded| seeded.shows("tasks-footer"));
  assert_eq!(seeded.keys(), ["KON-1"]);
  seeded.go(TaskList::new(TaskListTab::Done).with_search("retry"));
  assert_eq!(
    seeded.text("tasks-no-rows-title"),
    "No tasks in this tab.",
    "a tab without tasks says so, whatever the search"
  );
  assert!(!seeded.shows("tasks-clear-query"));
}

#[test]
fn tasks_a_search_that_hides_every_row_says_so_and_clear_brings_them_back() {
  let mut seeded = Seeded::tasks(
    "tasks",
    TaskList::new(TaskListTab::InProgress).with_search("retry budget per endpoint"),
  );
  assert_eq!(
    seeded.texts("tasks-no-rows"),
    [
      "No tasks in this tab match the search or pipeline filter.",
      "In progress has 8 tasks. Search looks at titles, keys and briefs.",
      "Clear search and filter"
    ]
  );
  assert!(!seeded.shows("tasks-footer"));
  seeded.click("tasks-clear-query");
  assert_eq!(
    seeded.window.navigation().route_now(),
    seeded.route(TaskList::new(TaskListTab::InProgress))
  );
  assert_eq!(seeded.keys().len(), 8);
}
