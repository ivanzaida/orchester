//! A team project's Task page, Modules list and Module page read from its server through the session's lurq queries
//! (ORC-717), on the QA app's `team-member`, `team-member-running` and `team-module-outage` states with the stand-in
//! team server's `/api/v2`: a member's Modules list naming who holds each module without New module, a member's Module
//! page with its claim, branch and tasks as text and no New task in module, a member's Task page with the task's
//! claim, assignee and module branch and nothing to edit, a change the server commits refreshing the mounted page, the
//! module's outage naming the module and what still runs here, and a local project's Modules left as they were.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_pages`

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{ModuleListTab, ModuleTab, ProjectPage, Route, TaskView},
  test_support::{Data, Window, project_id},
};
use orchester_core::{id::ModuleId, project::TaskKey};

use super::{prepared, team::State};

/// Webhooks v2, the first of the modules the QA app's server sets.
const WEBHOOKS: &str = "0191c7c0-0000-7000-8000-000000200000";

fn modules_route() -> Route {
  Route::Project(project_id(), ProjectPage::Modules(ModuleListTab::Open))
}

fn module_route() -> Route {
  let module: ModuleId = WEBHOOKS.parse().expect("a module id");
  Route::Project(project_id(), ProjectPage::Module(module, ModuleTab::Summary))
}

fn task_route(key: &str) -> Route {
  let key: TaskKey = key.parse().expect("a task key");
  Route::Project(project_id(), ProjectPage::task(key, TaskView::Summary, false))
}

/// The window on `route` in `state`, once `shown` shows.
fn on(state: State, route: Route, shown: &str) -> (Window, TeamServer) {
  let (mut window, server) = prepared(state);
  window.navigation().go(route);
  window.until(shown, |window| window.texts(shown).is_some());
  (window, server)
}

#[test]
fn team_qa_pages_a_members_modules_name_their_holders_without_new_module() {
  let (mut window, _server) = on(State::Member, modules_route(), "modules-row-2");
  assert!(window.texts("modules-new").is_none(), "a member creates no module");
  window.until("the Admins named", |window| {
    window
      .texts("modules-team-read-only-text")
      .is_some_and(|text| text.concat().contains("dana-k"))
  });
  assert_eq!(
    window.text("modules-team-read-only-text"),
    "Only Admins create and change modules: dana-k (Admin) and olha (Server admin). You claim a module to work on \
     all its tasks."
  );
  assert_eq!(window.text("modules-row-0-name"), "Webhooks v2");
  assert_eq!(window.text("modules-row-0-holder-name"), "olha");
  assert_eq!(window.text("modules-row-1-holder-name"), "Bohdan Koval (you)");
  assert_eq!(window.text("modules-row-2-holder-name"), "Not claimed");
  assert_eq!(window.text("modules-row-0-done"), "1 of 5 tasks done");
}

#[test]
fn team_qa_pages_a_members_module_page_reads_its_claim_branch_and_tasks() {
  let (mut window, _server) = on(State::Member, module_route(), "module-property-claim-value");
  assert_eq!(window.text("module-title"), "Webhooks v2");
  assert!(
    window.texts("module-new-task").is_none(),
    "a member makes no task in it"
  );
  assert!(window.texts("module-more").is_none(), "nothing to change");
  assert!(
    window
      .text("module-property-claim-value")
      .starts_with("olha · renewed ")
  );
  assert_eq!(window.text("module-property-assignee-value"), "olha");
  assert_eq!(window.text("module-property-branch-value"), "module/webhooks-v2");
  window.until("the module's tasks", |window| {
    window.texts("module-tasks-head-count").is_some()
  });
  assert_eq!(window.text("module-tasks-head-count"), "5");
  window.until("the Admins named", |window| {
    window
      .texts("module-team-read-only-text")
      .is_some_and(|text| text.concat().contains("dana-k"))
  });
}

#[test]
fn team_qa_pages_a_members_task_page_shows_its_claim_and_nothing_to_edit() {
  let (mut window, _server) = on(State::MemberRunning, task_route("KON-12"), "task-team-claim-title");
  assert_eq!(window.text("task-title"), "Add retry to webhook sender");
  assert_eq!(window.text("task-team-claim-title"), "Claimed by you");
  assert_eq!(window.text("task-property-assignee-label"), "Bohdan Koval (you)");
  assert_eq!(window.text("task-property-claim-label"), "You · this computer");
  assert_eq!(window.text("task-property-module-label"), "Webhooks v2");
  assert_eq!(window.text("task-property-merges-label"), "module/webhooks-v2");
  assert!(
    window.texts("task-module-field").is_none(),
    "the module is the server's"
  );
  // The holder's one line names who cancels and reopens; the page's own read-only line steps aside for it.
  window.until("the Admins named", |window| {
    window
      .texts("task-team-lifecycle-lock-text")
      .is_some_and(|text| text.concat().contains("dana-k"))
  });
  assert!(window.texts("task-team-read-only").is_none());
  window.press("task-more");
  window.until("the menu", |window| window.texts("task-more-panel").is_some());
  assert!(window.texts("task-edit").is_none(), "a member does not edit");
  assert!(window.texts("task-copy-link").is_some());
}

#[test]
fn team_qa_pages_a_change_the_server_commits_refreshes_the_task_page() {
  let (mut window, server) = on(State::Member, task_route("KON-12"), "task-team-claim-title");
  server.retitle(12, "Retry webhook sends with backoff");
  window.until("the new title", |window| {
    window
      .texts("task-title")
      .is_some_and(|title| title.concat() == "Retry webhook sends with backoff")
  });
}

#[test]
fn team_qa_pages_a_task_the_server_lacks_is_not_found() {
  let (mut window, _server) = on(State::Member, task_route("KON-99"), "task-not-found");
  assert!(window.text("task-not-found").contains("KON-99"));
}

#[test]
fn team_qa_pages_a_module_outage_names_the_module_and_keeps_no_data_as_current() {
  let (mut window, _server) = on(State::ModuleOutage, module_route(), "module-property-claim-value");
  window.until("the outage", |window| {
    window.texts("module-team-unavailable-title").is_some()
  });
  assert_eq!(
    window.text("module-team-unavailable-title"),
    "Webhooks v2 can't be read now"
  );
  assert_eq!(window.text("module-title"), "Webhooks v2");
  assert!(
    window.texts("module-property-claim-value").is_none(),
    "no claim shown as current"
  );
  assert!(window.texts("module-tasks-head").is_none(), "no task shown as current");
  assert!(
    window
      .text("module-team-unavailable-since")
      .contains("Team data unavailable since")
  );
}

#[test]
fn team_qa_pages_a_local_projects_modules_keep_new_module() {
  let data = Data::new();
  let launch = data.launch();
  data.add_kontur(launch.workspace().expect("the session started"));
  let mut window = Window::of(data, launch);
  window.navigation().go(modules_route());
  window.until("the modules", |window| window.texts("modules-empty").is_some());
  assert!(
    window.texts("modules-empty-new").is_some(),
    "a local project makes modules"
  );
  assert!(window.texts("modules-team-read-only").is_none());
}
