//! Where the Overview's actions lead: a tab is the route's, a row opens its task's Details and a run its Summary, a
//! *Needs you* card opens its task where it is decided, pointing at its decision card as the Needs you panel
//! does, *View all tasks* the Tasks list, Import resources its page on All and New task its form, and the project's
//! memory records its Memory page. A request whose task cannot be read has nothing to open: it stays locked and
//! goes nowhere.

use orchester_app::routes::{
  ImportTab, MemoryTab, OverviewTab, ProjectPage, Route, TaskList, TaskListTab, TaskTab, TaskView,
};
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
    task(&seeded, &key, TaskTab::default()),
    "a row opens its task on the tab a task opens on"
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
  let key = &first.task().expect("the first card's task").key;
  assert_eq!(seeded.window.navigation().route_now(), task(&seeded, key, tab));
}

#[test]
fn overview_keeps_a_tool_request_whose_run_cannot_be_read_with_its_action_locked() {
  let mut seeded = Seeded::damaging_approval_run("task-states");
  let tasks = seeded.tasks();
  let items = attention::items(&tasks);
  let unknown: Vec<usize> = items
    .iter()
    .enumerate()
    .filter(|(_, item)| item.kind == NeedKind::Approval && item.task().is_none())
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
  assert_eq!(
    seeded.text(&format!("{card}-reason-text")),
    "Its task can't be read, so there is nothing to open.",
    "as the Needs you panel says it"
  );
  seeded.click(&format!("{card}-action"));
  assert_eq!(
    seeded.window.navigation().route_now(),
    seeded.route(OverviewTab::Running),
    "there is no task to open"
  );
}

/// The element `id` is ringed as the card a navigation pointed at.
fn pointed(seeded: &mut Seeded, id: &str) -> bool {
  seeded
    .window
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| element.has_class("pointed"))
}

#[test]
fn overview_a_plan_and_a_limit_open_at_their_card_ringed_as_from_the_panel() {
  for (kind, card) in [("plan", "plan-review"), ("limit", "raise-limit")] {
    let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
    let action = seeded
      .ids("overview-need-")
      .into_iter()
      .find(|id| id.ends_with(&format!("-{kind}-action")))
      .unwrap_or_else(|| panic!("a {kind} waits in the seed"));
    let key = action
      .trim_start_matches("overview-need-")
      .split('-')
      .skip(1)
      .take(2)
      .collect::<Vec<_>>()
      .join("-");
    seeded.click(&action);
    assert_eq!(
      seeded.window.navigation().route_now(),
      task(&seeded, &key, TaskTab::Summary)
    );
    seeded.until(&format!("the {kind}'s card ringed"), |seeded| pointed(seeded, card));
  }
}

#[test]
fn overview_memory_records_card_opens_the_project_memory_page() {
  let mut seeded = Seeded::overview("tasks-memory", OverviewTab::Running);
  let tasks = seeded.tasks();
  let items = attention::items(&tasks);
  let index = items
    .iter()
    .position(|item| item.kind == NeedKind::Memory)
    .expect("the records wait as one item");
  assert_eq!(seeded.text("overview-needs-count"), items.len().to_string());
  let card = format!("overview-need-{index}-unknown-memory");
  let said = seeded.texts(&card);
  for line in ["Memory", "2 memory records wait for approval"] {
    assert!(said.iter().any(|said| said == line), "{line:?} in {said:?}");
  }
  assert_eq!(seeded.text(&format!("{card}-action")), "Review records");
  assert!(!seeded.shows(&format!("{card}-reason")), "the review is built");
  seeded.click(&format!("{card}-action"));
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::Memory(Some(MemoryTab::Waiting)))
  );
}

#[test]
fn overview_needs_you_tab_lists_the_memory_records_after_its_tasks_and_opens_their_page() {
  let mut seeded = Seeded::overview("tasks-memory", OverviewTab::NeedsYou);
  let tasks = seeded.tasks();
  let count = BoardTab::NeedsYou.count(&tasks);
  assert_eq!(count, tasks.needs_you(), "the tab counts what every Needs you counts");
  let label = seeded.text("overview-tab-needs_you-label");
  assert!(label.ends_with(&format!(" {count}")), "{label}");
  let rows = seeded.ids("overview-row-");
  assert_eq!(rows.len(), BoardTab::NeedsYou.rows(&tasks, 6, ", ").len());
  assert_eq!(
    seeded.text("overview-memory-row-title"),
    "2 memory records wait for approval"
  );
  assert_eq!(seeded.text("overview-memory-row-detail"), "Not a task: Kontur memory");
  assert_eq!(seeded.text("overview-memory-row-action"), "Review records");
  assert!(!seeded.shows("overview-memory-row-reason"), "the review is built");
  seeded.click("overview-memory-row-action");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::Memory(Some(MemoryTab::Waiting)))
  );
}

#[test]
fn overview_other_tabs_have_no_memory_row() {
  let mut seeded = Seeded::overview("tasks-memory", OverviewTab::Running);
  assert!(!seeded.shows("overview-memory-row"));
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
    Route::Project(seeded.project, ProjectPage::Import(ImportTab::All)),
    "Import resources opens on All"
  );
  seeded.go(OverviewTab::Running);
  seeded.click("overview-new-task");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::NewTask)
  );
}
