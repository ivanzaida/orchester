//! *Jump to* (`Jump to · open` `aDYAX`): the row under the switcher and Ctrl+K (Cmd+K) open it; it finds a task by
//! key, a module by name, a page and a hub tab, and actions; Enter opens the current result, Ctrl+Enter in a new tab,
//! a press the one pressed, and Esc closes it.

use orchester_app::{
  routes::{ModuleTab, ProjectPage, Route, SkillsFilter, TaskView},
  shell::sidebar::place::Place,
};
use orchester_core::id::ProjectId;

use super::{command, key, seeded, support::Window};

/// Opens *Jump to* with the row under the switcher and types `query`.
fn search(window: &mut Window, query: &str) {
  window.press("sidebar-jump");
  window.until("Jump to", |window| window.texts("jump-to").is_some());
  window.set("jump-to-field", query);
}

/// The first task's key and the first module's id and name, as the project's feed read them.
fn first_task_and_module(window: &mut Window, project: ProjectId) -> (String, orchester_core::id::ModuleId, String) {
  window.until("the tasks to be read", |window| {
    window
      .projects()
      .feed_now(project)
      .is_some_and(|feed| feed.tasks.get_untracked().ready().is_some())
  });
  let feed = window.projects().feed_now(project).expect("the feed");
  let tasks = feed.tasks.get_untracked();
  let tasks = tasks.ready().expect("the tasks are read");
  let task = tasks.tasks.first().expect("the scenario has tasks");
  let module = tasks.modules.first().expect("the scenario has modules");
  (task.key().to_string(), module.id(), module.name().as_str().to_owned())
}

#[test]
fn sidebar_jump_to_opens_a_task_a_module_a_page_and_a_hub_tab() {
  let (mut window, project) = seeded("modules");
  let navigation = window.navigation();
  let (task, module, name) = first_task_and_module(&mut window, project);
  // A task by its key, opened with Enter.
  search(&mut window, &task);
  let row = format!("jump-to-tasks-task-{task}");
  window.until(&row, |window| window.texts(&row).is_some());
  key(&mut window, ("Enter", "Enter"));
  let key = task.parse().expect("a task key");
  assert_eq!(
    navigation.route_now(),
    Route::Project(project, ProjectPage::Task(key, TaskView::default()))
  );
  assert!(window.texts("jump-to").is_none(), "a choice closes it");
  // A module by its name, and its Burndown tab, opened with a press.
  search(&mut window, &name);
  let burndown = format!("jump-to-modules-module-{module}-burndown");
  window.until(&burndown, |window| window.texts(&burndown).is_some());
  window.press(&burndown);
  assert_eq!(
    navigation.route_now(),
    Route::Project(project, ProjectPage::Module(module, ModuleTab::Burndown))
  );
  // A page, and a hub's tab.
  search(&mut window, "merge");
  window.press("jump-to-pages-merge-queue");
  assert_eq!(navigation.route_now(), Route::Project(project, ProjectPage::MergeQueue));
  search(&mut window, "skills");
  window.press("jump-to-pages-agents-skills");
  assert_eq!(
    navigation.route_now(),
    Route::Project(project, ProjectPage::Skills(SkillsFilter::default()))
  );
}

#[test]
fn sidebar_jump_to_opens_with_its_keys_moves_with_arrows_and_opens_in_a_new_tab() {
  let (mut window, project) = seeded("modules");
  let navigation = window.navigation();
  command(&mut window, ("k", "KeyK"));
  window.until("Jump to", |window| window.texts("jump-to").is_some());
  key(&mut window, ("Escape", "Escape"));
  window.until("Jump to to close", |window| window.texts("jump-to").is_none());
  command(&mut window, ("k", "KeyK"));
  window.until("Jump to again", |window| window.texts("jump-to").is_some());
  window.set("jump-to-field", "new");
  window.until("the actions", |window| window.texts("jump-to-group-actions").is_some());
  assert!(
    window.texts("jump-to-actions-new-task").is_some(),
    "New task is an action"
  );
  // ↓ moves the current result; the footer counts what shows.
  key(&mut window, ("ArrowDown", "ArrowDown"));
  assert!(window.text("jump-to-count").ends_with("results"));
  window.set("jump-to-field", "pipelines");
  let page = "jump-to-pages-pipelines";
  window.until(page, |window| window.texts(page).is_some());
  let tabs = navigation.tabs_now().tabs().len();
  let before = navigation.route_now();
  command(&mut window, ("Enter", "Enter"));
  let now = navigation.tabs_now();
  assert_eq!(now.tabs().len(), tabs + 1, "Ctrl+Enter opens it in a new tab");
  assert_eq!(
    *now.active().route(),
    Place::Pipelines.route(project),
    "the new tab, on Pipelines, is active"
  );
  assert!(
    now.tabs().iter().any(|tab| *tab.route() == before),
    "the tab it was opened from stays where it was"
  );
}
