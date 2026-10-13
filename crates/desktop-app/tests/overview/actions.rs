//! Where the Overview's actions lead: a tab is the route's, a row and a run open their task's Summary, a
//! *Needs you* card opens its task where it is decided, *View all tasks* the Tasks list and New task its
//! form. Import resources is not built: it stays locked and goes nowhere.

use orchester_app::routes::{OverviewTab, ProjectPage, Route, TaskList, TaskListTab, TaskTab, TaskView};
use orchester_core::project::TaskKey;
use orchester_workspace::tasks::{
  attention::{self, NeedKind},
  board::BoardTab,
};

use crate::harness::Seeded;

fn task(seeded: &Seeded, key: &str, tab: TaskTab) -> Route {
  let key: TaskKey = key.parse().expect("a task key");
  Route::Project(seeded.project, ProjectPage::Task(key, TaskView::of(tab)))
}

#[test]
fn overview_tabs_are_the_routes_and_show_their_tasks() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let tasks = seeded.tasks();
  seeded.click("overview-tab-needs_you");
  assert_eq!(
    seeded.window.navigation().route_now(),
    seeded.route(OverviewTab::NeedsYou)
  );
  let shown = seeded.ids("overview-row-");
  let expected: Vec<String> = BoardTab::NeedsYou
    .rows(&tasks, 6, ", ")
    .iter()
    .map(|row| format!("overview-row-{}", row.key))
    .collect();
  assert_eq!(shown, expected);
  seeded.go(OverviewTab::Queued);
  let queued = BoardTab::Queued.count(&tasks);
  assert_eq!(seeded.ids("overview-row-").len(), queued.min(6));
  if queued == 0 {
    assert_eq!(seeded.text("overview-tasks-empty"), "No task is in Queued.");
  }
}

#[test]
fn overview_rows_runs_and_cards_open_their_task() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let tasks = seeded.tasks();
  let row = seeded.ids("overview-row-").first().cloned().expect("a running task");
  seeded.click(&row);
  let key = row.trim_start_matches("overview-row-").to_owned();
  assert_eq!(
    seeded.window.navigation().route_now(),
    task(&seeded, &key, TaskTab::Summary)
  );

  seeded.go(OverviewTab::Running);
  let event = seeded.ids("overview-event-").first().cloned().expect("a recent run");
  seeded.click(&event);
  let key = event.trim_start_matches("overview-event-").to_owned();
  assert_eq!(
    seeded.window.navigation().route_now(),
    task(&seeded, &key, TaskTab::Summary)
  );

  seeded.go(OverviewTab::Running);
  let items = attention::items(&tasks);
  let first = items.first().expect("something waits for the person");
  let tab = match first.kind {
    NeedKind::Approval | NeedKind::Question | NeedKind::Input => TaskTab::Conversation,
    _ => TaskTab::Summary,
  };
  let action = seeded
    .ids("overview-need-0-")
    .into_iter()
    .find(|id| id.ends_with("-action"))
    .expect("the first card's action");
  seeded.click(&action);
  let key = &first.task.as_ref().expect("the first card's task").key;
  assert_eq!(seeded.window.navigation().route_now(), task(&seeded, key, tab));
}

#[test]
fn overview_keeps_a_tool_request_whose_run_cannot_be_read_without_an_action() {
  let mut seeded = Seeded::damaging_approval_run("task-states");
  let tasks = seeded.tasks();
  let items = attention::items(&tasks);
  let unknown: Vec<usize> = items
    .iter()
    .enumerate()
    .filter(|(_, item)| item.kind == NeedKind::Approval && item.task.is_none())
    .map(|(index, _)| index)
    .collect();
  let [index] = unknown.as_slice() else {
    panic!("one request without its task: {items:?}");
  };
  assert_eq!(seeded.text("overview-needs-count"), items.len().to_string());
  let card = format!("overview-need-{index}-unknown-approval");
  let said = seeded.texts(&card);
  assert!(
    said
      .iter()
      .any(|line| line == "Its run cannot be read, so its task is not known"),
    "{said:?}"
  );
  assert!(!seeded.shows(&format!("{card}-action")), "there is no task to open");
}

#[test]
fn overview_view_all_and_new_task_lead_to_their_screens() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  seeded.click("overview-view-all");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::Tasks(TaskList::new(TaskListTab::All)))
  );
  seeded.go(OverviewTab::Running);
  seeded.click("overview-import");
  assert_eq!(
    seeded.window.navigation().route_now(),
    seeded.route(OverviewTab::Running),
    "Import resources is locked"
  );
  seeded.click("overview-new-task");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::NewTask)
  );
}
