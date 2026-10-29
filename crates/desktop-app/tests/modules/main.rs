//! The Modules screens (ORC-48, redesigned in ORC-112) on seeded Kontur projects whose modules are made before the
//! window opens: the list on its views with its search (`list`); a module's page with its tabs and their routes, its
//! Summary's grouped tasks and in-place properties, its burndown, its *More actions* and the task page's header
//! geometry (`page`); New module and Edit module as pages (`forms`); and here the module filter of Tasks, the module
//! field of the task's header and of the task form, and the Delivery view beside Usage forecasting an open module
//! (ORC-50). Needs `git`, as the seed does.

mod forms;
mod harness;
#[allow(
  dead_code,
  reason = "the seed's scenarios are shared with its own example; these tests use a part"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
mod list;
mod page;
#[path = "../support/mod.rs"]
mod support;

use harness::{Seeded, details, module_with};
use orchester_app::routes::{ProjectPage, Route, TaskList, TaskListTab, TaskView};
use orchester_core::{
  task::organization::{ChangeActor, WorkflowStatus},
  time::Timestamp,
};
use orchester_workspace::delivery::DeliveryPeriod;

#[test]
fn modules_tasks_filter_keeps_the_tasks_of_a_module_or_of_none() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    let picks = [(WorkflowStatus::Todo, 2)];
    picked = Some(module_with(db, details("Checkout", (0, 7)), &picks).1);
  });
  let mut inside = picked.expect("the module's tasks");
  inside.sort();
  seeded.open(ProjectPage::Tasks(TaskList::new(TaskListTab::All)), "tasks-filter");
  let all = |count: usize| move |seeded: &mut Seeded| seeded.text("tasks-view-all") == format!("All {count}");
  seeded.until("the tasks", all(24));
  seeded.click("tasks-filter");
  seeded.until("the filter menu", |seeded| seeded.shows("tasks-filter-module"));
  assert_eq!(
    [0, 1].map(|index| seeded.text(&format!("tasks-filter-module-{index}"))),
    ["Checkout", "No module"],
    "the Filter offers the project's modules and none"
  );
  seeded.click("tasks-filter-module-0");
  seeded.until("the module's tasks", all(inside.len()));
  assert_eq!(rows_of_tasks(&mut seeded), inside);
  seeded.click("tasks-filter-module-0");
  seeded.click("tasks-filter-module-1");
  seeded.until("the tasks in no module", all(22));
  assert!(rows_of_tasks(&mut seeded).iter().all(|key| !inside.contains(key)));
  seeded.click("tasks-filter-module-1");
  seeded.until("every task again", all(24));
}

/// The keys of the Tasks list's rows, sorted.
fn rows_of_tasks(seeded: &mut Seeded) -> Vec<String> {
  seeded.ids("task-key-")
}

#[test]
fn modules_the_task_header_moves_the_task_as_a_person() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[]).0);
  });
  let module = picked.expect("the module");
  let key = "KON-12";
  assert_eq!(seeded.task(key).module(), None);
  let page = ProjectPage::Task(key.parse().expect("a key"), TaskView::default());
  seeded.open(page, "task-module");
  assert_eq!(seeded.options("task-module"), ["No module", "Checkout"]);
  seeded.choose("task-module", "Checkout");
  seeded.until("the move", |seeded| seeded.task(key).module() == Some(module));
  let moves = seeded.db().module_changes(module).expect("the moves");
  let last = moves.last().expect("the move is kept");
  assert_eq!(
    (last.from, last.to, last.actor),
    (None, Some(module), ChangeActor::Person)
  );
  assert!(!seeded.shows("task-module-error"));
  // The saved task read again reaches the field with the module it chose (issue #64: the window froze here).
  seeded.read_again();
  seeded.until("the header keeps the module", |seeded| {
    seeded.text("task-module").contains("Checkout")
  });
}

/// A move made elsewhere (another window, MCP, the team server) while the task's page is open: the header shows
/// the new module and the window keeps working. It once deadlocked here, the field holding its own lock while it
/// set the choice whose watcher took that lock again.
#[test]
fn modules_the_task_header_follows_a_move_made_elsewhere() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[]).0);
  });
  let module = picked.expect("the module");
  let key = "KON-12";
  let page = ProjectPage::Task(key.parse().expect("a key"), TaskView::default());
  seeded.open(page, "task-module");
  let task = seeded.task(key).id();
  seeded
    .db()
    .set_task_module(task, Some(module), ChangeActor::Orchester, Timestamp::now())
    .expect("moved elsewhere");
  seeded.read_again();
  seeded.until("the header shows the module", |seeded| {
    seeded.text("task-module").contains("Checkout")
  });
  let moves = seeded.db().module_changes(module).expect("the moves");
  assert_eq!(moves.len(), 1, "the header made no move of its own: {moves:?}");
}

#[test]
fn modules_the_task_form_saves_the_chosen_module() {
  let mut picked = None;
  let mut seeded = Seeded::new("live", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[]).0);
  });
  let module = picked.expect("the module");
  seeded.open(
    ProjectPage::EditTask("KON-1".parse().expect("a key")),
    "task-form-module",
  );
  assert_eq!(seeded.text("task-form-module-value"), "None");
  seeded.click("task-form-module");
  seeded.until("the module menu", |seeded| seeded.shows("task-form-module-menu"));
  assert_eq!(seeded.text("task-form-module-0"), "No module");
  assert_eq!(seeded.text("task-form-module-1"), "Checkout");
  seeded.click("task-form-module-1");
  assert_eq!(seeded.text("task-form-module-value"), "Checkout");
  seeded.click("task-form-save");
  seeded.until("the save", |seeded| seeded.task("KON-1").module() == Some(module));
}

#[test]
fn modules_delivery_view_forecasts_the_open_module_and_reads_its_charts_as_tables() {
  let mut seeded = Seeded::new("tasks", |db| {
    module_with(db, details("Checkout", (3, 4)), &[(WorkflowStatus::Todo, 2)]);
  });
  seeded.open(ProjectPage::Delivery(DeliveryPeriod::Month), "delivery-forecast-table");
  assert!(seeded.text("delivery-forecast-row-0").contains("Queued tasks"));
  let module = seeded.text("delivery-forecast-row-1");
  assert!(
    module.contains("Checkout") && module.contains("Not enough history yet"),
    "the module's two open tasks, without a date while history is short: {module}"
  );
  assert!(seeded.text("delivery-forecast-rule").contains("30 whole days"));
  for card in [
    "delivery-throughput",
    "delivery-cycle",
    "delivery-time",
    "delivery-flow",
  ] {
    assert!(seeded.shows(card), "{card} is shown");
  }
  assert!(seeded.shows("delivery-flow-legend"), "the flow's chart shows first");
  seeded.click("delivery-flow-view-table");
  assert!(seeded.shows("delivery-flow-table") && !seeded.shows("delivery-flow-legend"));
  seeded.click("delivery-period-90d");
  assert_eq!(
    seeded.route(),
    Route::Project(seeded.project, ProjectPage::Delivery(DeliveryPeriod::Quarter))
  );
  seeded.click("usage-view-usage");
  assert_eq!(
    seeded.route(),
    Route::Project(seeded.project, ProjectPage::Usage(Box::default()))
  );
}
