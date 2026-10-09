//! Tab labels and what needs the person: a tab names its project and page, and a tab of a project where
//! something waits ends in an amber hand; the title bar's pill counts what waits in every project the tabs
//! show, 0 included. The project is the QA seed's `tasks` scenario (4 need you). Needs `git`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed reports what it wrote; these tests read the window instead"
)]
mod kontur;
mod support;

use std::time::{Duration, Instant};

use kontur::{SeedRequest, scenarios};
use lurq::app::events::MouseButton;
use orchester_app::{
  routes::{OverviewTab, ProjectPage, Route},
  ui::tooltip::TOOLTIP_DELAY,
};
use orchester_core::{id::ProjectId, time::Timestamp};
use support::{Data, Window, add_project, project_id};

/// The window on the `tasks` scenario, landed on Kontur's Overview once its tasks are read; `atlas`, when
/// given, is a second project in the list, with no task.
fn seeded_window(atlas: Option<ProjectId>) -> Window {
  let data = Data::new();
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join("repo-tasks"),
    scenario: scenarios::named("tasks").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  kontur::seed(&request).expect("the scenario seeds");
  let launch = data.launch();
  if let Some(atlas) = atlas {
    let workspace = launch.workspace().expect("the session started");
    add_project(workspace, (atlas, "ATL", "Atlas"), &data.repo("repo-atlas"));
    // Kontur stays the project used last, where the window opens.
    workspace
      .open(&data.dir.path().join("repo-tasks"))
      .expect("Kontur opens");
  }
  let mut window = Window::of(data, launch);
  window.until("Kontur's tasks to be read", |window| {
    window.texts("needs-you-pill").is_some()
  });
  window
}

#[test]
fn tab_labels_mark_a_project_that_needs_you() {
  let mut window = seeded_window(None);
  assert_eq!(window.texts("tab-1"), Some(vec!["Kontur · Overview".to_owned()]));
  assert_eq!(
    tooltip(&mut window, "tab-1-attention"),
    "Kontur · 4 need you",
    "Kontur has 4 things waiting"
  );
  assert_eq!(window.texts("needs-you-pill"), Some(vec!["4 need you".to_owned()]));
  let navigation = window.navigation();
  navigation.open_tab();
  navigation.go(Route::Settings);
  window.pass();
  assert_eq!(window.texts("tab-2"), Some(vec!["Settings".to_owned()]));
  assert!(window.texts("tab-2-attention").is_none(), "no project, no mark");
  assert_eq!(
    window.texts("needs-you-pill"),
    Some(vec!["4 need you".to_owned()]),
    "the pill counts every project the tabs show"
  );
}

#[test]
fn tab_labels_pill_stays_at_zero_and_opens_the_overviews_needs_you() {
  let mut window = Window::open();
  let navigation = window.navigation();
  assert!(
    window.texts("needs-you-pill").is_none(),
    "no project shown, nothing counted"
  );
  navigation.go(Route::Project(
    project_id(),
    ProjectPage::Overview(OverviewTab::Running),
  ));
  window.until("Kontur's tasks to be read", |window| {
    window.texts("needs-you-pill").is_some()
  });
  assert_eq!(window.texts("needs-you-pill"), Some(vec!["0 need you".to_owned()]));
  let pill = window
    .tree
    .get_element_by_id_mut("needs-you-pill")
    .and_then(|element| element.bounds())
    .expect("the pill is laid out");
  assert_eq!(pill.height, 26.0, "as tall as the design's pill (`O3eKK`)");
  window.until("the runner to give up", |window| {
    window.texts("tab-1-attention").is_some()
  });
  assert_eq!(
    tooltip(&mut window, "tab-1-attention"),
    "Kontur · runner offline",
    "nothing waits, but the tests' runner never starts: an unresolved failure"
  );
  navigation.open_tab();
  window.pass();
  let bounds = window
    .tree
    .get_element_by_id_mut("needs-you-pill")
    .and_then(|element| element.bounds())
    .expect("the pill is laid out");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
  assert_eq!(
    navigation.route_now(),
    Route::Project(project_id(), ProjectPage::Overview(OverviewTab::NeedsYou)),
    "from a tab without a project, the pill opens the first project's Needs you"
  );
}

#[test]
fn tab_labels_pill_sums_every_open_project_and_splits_it_in_its_tooltip() {
  let atlas = ProjectId::generate();
  let mut window = seeded_window(Some(atlas));
  let navigation = window.navigation();
  navigation.open_tab();
  navigation.go(Route::Project(atlas, ProjectPage::Overview(OverviewTab::Running)));
  window.until("Atlas's tasks to be read", |window| {
    window.texts("tab-2").is_some_and(|texts| texts == ["Atlas · Overview"])
      && window.texts("tab-2-attention").is_some()
  });
  assert_eq!(window.texts("needs-you-pill"), Some(vec!["4 need you".to_owned()]));
  assert_eq!(tooltip(&mut window, "needs-you-pill"), "Kontur 4 · Atlas 0");
}

/// The words of the tooltip of the element `id`, once the pointer rested on it.
fn tooltip(window: &mut Window, id: &str) -> String {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  window
    .tree
    .mouse_move(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.pass();
  window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  window.pass();
  let panel = format!("{id}-tooltip");
  let words = window.texts(&panel).expect("the tooltip shows").join("");
  window.tree.mouse_move(0.0, 0.0);
  window.pass();
  words
}
