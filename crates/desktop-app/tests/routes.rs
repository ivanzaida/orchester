//! Every route has a path that reads back as the same route and that the router's table resolves to its
//! screen; paths that name nothing parse to nothing and fall back in the table.

use orchester_app::routes::{ProjectPage, Route, Screen, TaskTab, routes};
use orchester_core::{id::ProjectId, project::TaskKey};

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";

fn project() -> ProjectId {
  PROJECT.parse().expect("a project id")
}

fn key() -> TaskKey {
  "KON-12".parse().expect("a task key")
}

/// One route of every screen, a task once per tab.
fn every_route() -> Vec<Route> {
  let key = key();
  let mut all: Vec<Route> = Screen::ALL
    .into_iter()
    .filter_map(|screen| match screen {
      Screen::AllProjects => Some(Route::AllProjects),
      Screen::Settings => Some(Route::Settings),
      screen => ProjectPage::of(screen).map(|page| Route::Project(project(), page)),
    })
    .collect();
  all.extend(
    TaskTab::ALL
      .into_iter()
      .map(|tab| Route::Project(project(), ProjectPage::Task(key.clone(), tab))),
  );
  all
}

#[test]
fn every_screen_has_a_route() {
  let screens: Vec<Screen> = every_route().iter().map(Route::screen).collect();
  for screen in Screen::ALL {
    assert!(screens.contains(&screen), "{screen:?} has no route");
  }
}

#[test]
fn every_route_reads_back_from_its_path() {
  for route in every_route() {
    assert_eq!(Route::parse(&route.path()), Some(route.clone()), "{}", route.path());
  }
}

#[test]
fn paths_are_the_documented_ones() {
  let key = key();
  let paths = [
    (Route::AllProjects, "/"),
    (Route::Settings, "/settings"),
    (
      Route::Project(project(), ProjectPage::Overview),
      "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b",
    ),
    (
      Route::Project(project(), ProjectPage::Tasks),
      "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/tasks",
    ),
    (
      Route::Project(project(), ProjectPage::Task(key.clone(), TaskTab::Conversation)),
      "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/tasks/KON-12",
    ),
    (
      Route::Project(project(), ProjectPage::Task(key, TaskTab::Changes)),
      "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/tasks/KON-12/changes",
    ),
    (
      Route::Project(project(), ProjectPage::PullRequests),
      "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/pull-requests",
    ),
    (
      Route::Project(project(), ProjectPage::Settings),
      "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/settings",
    ),
  ];
  for (route, path) in paths {
    assert_eq!(route.path(), path);
  }
}

#[test]
fn the_router_table_resolves_every_route_to_its_screen() {
  let table = routes();
  for route in every_route() {
    let path = route.path();
    let chain = table.resolve(&path);
    let leaf = chain.last().unwrap_or_else(|| panic!("{path} resolves"));
    assert!(
      route
        .screen()
        .patterns()
        .iter()
        .any(|pattern| pattern == leaf.pattern_raw()),
      "{path} resolves to {} instead of {:?}",
      leaf.pattern_raw(),
      route.screen().patterns()
    );
  }
}

#[test]
fn paths_that_name_nothing_parse_to_nothing_and_fall_back_in_the_router() {
  let table = routes();
  for path in [
    "",
    "settings",
    "/nope",
    "/settings/general",
    "/project",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/nope",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/tasks/KON-12/nope",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/tasks/KON-12/summary/more",
    "/project/not-an-id",
    "/project/../tasks",
  ] {
    assert_eq!(Route::parse(path), None, "{path:?}");
  }
  let screens: Vec<String> = Screen::ALL.into_iter().flat_map(Screen::patterns).collect();
  for path in [
    "/nope",
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/nope",
    "/settings/general",
  ] {
    let chain = table.resolve(path);
    let leaf = chain.last().unwrap_or_else(|| panic!("{path} falls back"));
    assert!(
      !screens.iter().any(|pattern| pattern == leaf.pattern_raw()),
      "{path} names no screen"
    );
  }
}

#[test]
fn paths_name_projects_by_id_and_tasks_by_key() {
  assert_eq!(Route::parse("/project/kontur"), None, "a project is named by its id");
  assert_eq!(
    Route::parse(&format!("/project/{PROJECT}/tasks/KON-0")),
    None,
    "task numbers start at 1"
  );
  assert_eq!(
    Route::parse(&format!("/project/{PROJECT}/tasks/kon-12")),
    None,
    "project keys are uppercase"
  );
  let task = Route::Project(project(), ProjectPage::Task(key(), TaskTab::Summary));
  assert_eq!(task.path(), format!("/project/{PROJECT}/tasks/KON-12/summary"));
}

#[test]
fn screens_and_tabs_read_back_from_their_names() {
  for screen in Screen::ALL {
    assert_eq!(Screen::from_name(screen.name()), Some(screen));
  }
  for tab in TaskTab::ALL {
    assert_eq!(TaskTab::from_name(tab.name()), Some(tab));
  }
  assert_eq!(Screen::from_name("dashboard"), None);
}

#[test]
fn a_task_selects_tasks_in_the_sidebar() {
  let key = key();
  let task = Route::Project(project(), ProjectPage::Task(key, TaskTab::Summary));
  assert_eq!(task.screen(), Screen::Task);
  assert_eq!(task.nav_screen(), Screen::Tasks);
  assert_eq!(task.project(), Some(project()));
  assert_eq!(Route::Settings.project(), None);
}
