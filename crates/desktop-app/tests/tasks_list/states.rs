//! What the list shows instead of rows: a project without tasks, the tasks being read, reading them failing, a view
//! without tasks, and a search that hides every row. Loading and a failure never look like an empty list.

use orchester_app::{
  project::hold::ReadHold,
  routes::{ImportTab, ProjectPage, Route, TaskList, TaskListTab},
};

use crate::harness::Seeded;

/// Whether the list shows anything that says it has no tasks.
fn looks_empty(seeded: &mut Seeded) -> bool {
  seeded.shows("tasks-empty") || seeded.shows("tasks-no-rows") || seeded.shows("tasks-list")
}

#[test]
fn tasks_a_project_without_tasks_shows_its_title_and_new_task_and_import() {
  let mut seeded = Seeded::new("empty");
  seeded.go(TaskList::default());
  seeded.until("the empty project", |seeded| seeded.shows("tasks-empty"));
  assert_eq!(
    seeded.texts("tasks-empty"),
    [
      "No tasks yet",
      "A task is a piece of work a pipeline carries from brief to merge, in its own worktree.",
      "New task",
      "Import resources"
    ]
  );
  assert_eq!(seeded.texts("tasks-header"), ["Tasks"], "only the title");
  assert!(!seeded.shows("tasks-views"), "no view to choose");
  assert!(!seeded.shows("tasks-search"));
  seeded.click("tasks-empty-import");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::Import(ImportTab::All))
  );
  seeded.go(TaskList::default());
  seeded.until("the empty project", |seeded| seeded.laid_out("tasks-empty-new-task"));
  seeded.click("tasks-empty-new-task");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::NewTask)
  );
}

/// The seed's Tasks at `list` while its tasks are held from being read, and what lets them be read.
fn held(list: TaskList) -> (Seeded, ReadHold) {
  let mut seeded = Seeded::reading("tasks", true);
  let hold = seeded
    .window
    .launch
    .services()
    .expect("the session started")
    .hold
    .clone();
  seeded.go(list);
  seeded.until("the screen", |seeded| seeded.shows("tasks-loading"));
  (seeded, hold)
}

/// Waits, the window's timers firing as they fall due, until the skeleton `id` is laid out.
fn bones(seeded: &mut Seeded, id: &str) {
  seeded.until(id, |seeded| {
    seeded.window.tree.tick_timers();
    seeded.laid_out(id)
  });
}

#[test]
fn tasks_loading_shows_the_list_skeleton_after_a_moment_and_bones_for_counts_never_an_empty_list() {
  let (mut seeded, hold) = held(TaskList::default());
  assert!(seeded.shows("tasks-loading"));
  assert!(
    !seeded.shows("tasks-loading-rows"),
    "a fast reading never flashes the bones"
  );
  bones(&mut seeded, "tasks-loading-rows");
  assert_eq!(seeded.texts("tasks-loading-rows"), Vec::<String>::new(), "bones only");
  assert_eq!(seeded.view("all"), "All", "each count a bone");
  assert!(seeded.laid_out("tasks-view-all-count"));
  assert!(seeded.shows("tasks-search"), "the header stands over the skeleton");
  assert!(!looks_empty(&mut seeded));
  let top = seeded.bounds("tasks-loading").y;
  hold.set(false);
  seeded.until("the rows", |seeded| seeded.laid_out("tasks-list"));
  assert!(!seeded.shows("tasks-loading"));
  assert!(
    (seeded.bounds("tasks-list").y - top).abs() < 0.5,
    "the rows take the skeleton's place"
  );
}

#[test]
fn tasks_loading_on_the_board_shows_its_columns_heads_over_card_bones() {
  let (mut seeded, hold) = held(TaskList::new(TaskListTab::All));
  seeded.until("the layout switch", |seeded| seeded.laid_out("tasks-layout-board"));
  seeded.click("tasks-layout-board");
  bones(&mut seeded, "tasks-loading-board");
  assert!(!seeded.shows("tasks-loading-rows"));
  for (column, name) in [
    ("backlog", "Backlog"),
    ("todo", "Todo"),
    ("in_progress", "In progress"),
    ("in_review", "In review"),
    ("done", "Done"),
  ] {
    let head = format!("tasks-loading-column-{column}-name");
    assert_eq!(seeded.text(&head), name, "the real head of {column}");
    assert!(
      seeded.laid_out(&format!("tasks-loading-column-{column}-count")),
      "its count a bone"
    );
  }
  assert!(!seeded.shows("tasks-loading-column-canceled"));
  assert_eq!(
    seeded.text("tasks-loading-new-task"),
    "New task",
    "Backlog keeps its foot"
  );
  assert!(!looks_empty(&mut seeded));
  let top = seeded.bounds("tasks-loading-column-backlog-head").y;
  hold.set(false);
  seeded.until("the board", |seeded| seeded.laid_out("tasks-board-column-backlog-head"));
  assert!(!seeded.shows("tasks-loading"));
  assert!(
    (seeded.bounds("tasks-board-column-backlog-head").y - top).abs() < 0.5,
    "the columns take the skeleton's place"
  );
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
  assert_eq!(seeded.view("all"), "All —");
  assert!(!looks_empty(&mut seeded));
  let repaired = rusqlite::Connection::open(seeded.repo.join(".orchester").join("orchester.db"))
    .expect("the project database opens")
    .execute("UPDATE tasks SET status = 'draft'", [])
    .expect("the task is repaired");
  assert_eq!(repaired, 1);
  seeded.click("tasks-load-error-retry");
  seeded.until("the task read again", |seeded| seeded.shows("tasks-view-all"));
  seeded.go(TaskList::new(TaskListTab::All));
  seeded.until("the row", |seeded| seeded.keys() == ["KON-1"]);
  seeded.go(TaskList::new(TaskListTab::Done).with_search("retry"));
  assert_eq!(
    seeded.text("tasks-no-rows-title"),
    "No tasks in this view.",
    "a view without tasks says so, whatever the search"
  );
  assert!(!seeded.shows("tasks-clear-query"));
}

#[test]
fn tasks_a_search_that_hides_every_row_says_so_and_clear_brings_them_back() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default().with_search("retry budget per endpoint"));
  assert_eq!(
    seeded.texts("tasks-no-rows"),
    [
      "No tasks in this view match the search or filters.",
      "Active has 18 tasks. Search looks at titles, keys and briefs.",
      "Clear search and filters"
    ]
  );
  assert!(!seeded.shows("tasks-list"));
  seeded.click("tasks-clear-query");
  assert_eq!(
    seeded.window.navigation().route_now(),
    seeded.route(TaskList::default())
  );
  seeded.until("the rows", |seeded| seeded.keys().len() == 18);
}
