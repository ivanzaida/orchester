//! Every control that navigates opens its place by a browser's rules (`navigation::link`), as a chat's links do
//! (`support::link_press`): a middle click and Ctrl+click (Cmd+click on macOS) in a background tab right of the active
//! one, Ctrl+Shift+click in a new tab there it switches to, a plain click in the active tab as before. One control of
//! each kind, on the QA seed's project (`examples/seed`): a sidebar entry, a task row, a breadcrumb, an Overview
//! card, a status bar item and a project row of All projects; and an action inside a row that opens, which keeps a
//! middle click to itself. Needs `git`, as the seed does.

#[path = "../overview/harness.rs"]
#[allow(dead_code, reason = "the Overview's harness serves its own tests; these use a part")]
mod harness;
#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

use lurq::app::events::MouseButton;
use orchester_app::routes::{OverviewTab, ProjectPage, Route, TaskList, TaskTab, TaskView};
use orchester_core::project::TaskKey;
use orchester_workspace::tasks::attention::{self, NeedKind};

use self::{
  harness::Seeded,
  support::link_press::{Presser, control_opens_by_browser_rules, press_control},
};

fn task(seeded: &Seeded, key: &str, tab: TaskTab) -> Route {
  let key: TaskKey = key.parse().expect("a task key");
  Route::Project(seeded.project, ProjectPage::Task(key, TaskView::of(tab)))
}

fn tasks_list(seeded: &Seeded) -> Route {
  Route::Project(seeded.project, ProjectPage::Tasks(TaskList::default()))
}

#[test]
fn a_sidebar_entry_opens_its_screen_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let place = tasks_list(&seeded);
  control_opens_by_browser_rules(&mut seeded.window, "nav-tasks", &place);
}

#[test]
fn a_task_row_opens_its_task_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  seeded.window.navigation().go(tasks_list(&seeded));
  seeded.until("the task rows", |seeded| !seeded.ids("task-row-").is_empty());
  let row = seeded.ids("task-row-").first().cloned().expect("a task row");
  let place = task(&seeded, row.trim_start_matches("task-row-"), TaskTab::Summary);
  control_opens_by_browser_rules(&mut seeded.window, &row, &place);
}

#[test]
fn a_breadcrumb_opens_its_page_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let row = seeded.ids("overview-row-").first().cloned().expect("a running task");
  let page = task(&seeded, row.trim_start_matches("overview-row-"), TaskTab::Summary);
  seeded.window.navigation().go(page);
  // *Kontur / Tasks / KON-12*: the second crumb is the Tasks list.
  let place = tasks_list(&seeded);
  control_opens_by_browser_rules(&mut seeded.window, "task-crumbs-1", &place);
}

#[test]
fn an_overview_card_opens_its_task_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let first = attention::items(&seeded.tasks())
    .into_iter()
    .next()
    .expect("something waits for the person");
  let tab = match first.kind {
    NeedKind::Approval | NeedKind::Question | NeedKind::Input => TaskTab::Conversation,
    _ => TaskTab::Summary,
  };
  let key = first.task().expect("the first card's task").key.clone();
  let action = seeded
    .ids("overview-need-0-")
    .into_iter()
    .find(|id| id.ends_with("-action"))
    .expect("the first card's action");
  let place = task(&seeded, &key, tab);
  control_opens_by_browser_rules(&mut seeded.window, &action, &place);
}

#[test]
fn a_status_bar_item_opens_its_page_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let place = seeded.route(OverviewTab::Queued);
  control_opens_by_browser_rules(&mut seeded.window, "status-queued", &place);
}

#[test]
fn a_project_row_of_all_projects_opens_its_overview_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  seeded.window.navigation().go(Route::AllProjects);
  let row = format!("project-row-{}", seeded.project);
  seeded.until("the project's row", |seeded| seeded.shows(&row));
  let place = Route::Project(seeded.project, ProjectPage::Overview(OverviewTab::default()));
  control_opens_by_browser_rules(&mut seeded.window, &row, &place);
}

#[test]
fn a_middle_click_on_an_action_inside_a_row_that_opens_opens_nothing() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  seeded.window.navigation().go(Route::AllProjects);
  let remove = format!("project-remove-{}", seeded.project);
  seeded.until("the project's row", |seeded| seeded.shows(&remove));
  let navigation = seeded.window.navigation();
  let before = navigation.tabs_now();
  for presser in [Presser::Person, Presser::Synthetic] {
    press_control(
      &mut seeded.window,
      &remove,
      (presser, MouseButton::Middle),
      (false, false),
    );
    assert_eq!(
      navigation.tabs_now(),
      before,
      "{presser:?}: the row's project opens in no tab"
    );
  }
  assert_eq!(navigation.route_now(), Route::AllProjects);
}
