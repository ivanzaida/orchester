//! Tabs and history: each tab keeps its own Back/Forward history, new tabs open All projects, closing
//! follows the design's rules, the window shows the active tab's route, and the shortcuts ask for the
//! right moves.

mod support;

use lurq::app::events::MouseButton;
use orchester_app::{
  navigation::{
    shortcuts::{Keys, Shortcut, shortcut},
    tabs::TabSet,
  },
  routes::{ProjectPage, ProjectRef, Route, TaskKey, TaskTab},
};
use support::Window;

fn project(page: ProjectPage) -> Route {
  Route::Project(ProjectRef::new("kontur").expect("a valid project"), page)
}

fn task(tab: TaskTab) -> Route {
  project(ProjectPage::Task(TaskKey::new("KON-12").expect("a valid key"), tab))
}

#[test]
fn a_new_window_has_one_tab_at_all_projects_that_cannot_close() {
  let mut tabs = TabSet::default();
  let only = tabs.active().id();
  assert_eq!(tabs.tabs().len(), 1);
  assert_eq!(*tabs.active().route(), Route::AllProjects);
  assert!(!tabs.can_close(only));
  assert!(!tabs.close(only));
  assert!(!tabs.active().can_back() && !tabs.active().can_forward());
}

#[test]
fn going_somewhere_is_history_that_back_and_forward_walk() {
  let mut tabs = TabSet::default();
  assert!(tabs.go(project(ProjectPage::Overview)));
  assert!(tabs.go(project(ProjectPage::Tasks)));
  assert!(!tabs.go(project(ProjectPage::Tasks)), "the tab is there already");
  assert!(tabs.back());
  assert_eq!(*tabs.active().route(), project(ProjectPage::Overview));
  assert!(tabs.back());
  assert_eq!(*tabs.active().route(), Route::AllProjects);
  assert!(!tabs.back());
  assert!(tabs.forward());
  assert!(tabs.go(Route::Settings), "going from the middle drops what was ahead");
  assert!(!tabs.active().can_forward());
  assert!(tabs.back());
  assert_eq!(*tabs.active().route(), project(ProjectPage::Overview));
}

#[test]
fn each_tab_keeps_its_own_history() {
  let mut tabs = TabSet::default();
  let first = tabs.active().id();
  tabs.go(project(ProjectPage::Pipelines));
  let second = tabs.open();
  assert_eq!(
    *tabs.active().route(),
    Route::AllProjects,
    "a new tab opens All projects"
  );
  tabs.go(task(TaskTab::Summary));
  assert!(tabs.select(first));
  assert_eq!(*tabs.active().route(), project(ProjectPage::Pipelines));
  assert!(tabs.back());
  assert_eq!(*tabs.active().route(), Route::AllProjects);
  assert!(tabs.select(second));
  assert_eq!(*tabs.active().route(), task(TaskTab::Summary));
  assert!(!tabs.select(second), "the tab is active already");
}

#[test]
fn a_new_tab_opens_right_of_the_active_one() {
  let mut tabs = TabSet::default();
  let first = tabs.active().id();
  let last = tabs.open();
  tabs.select(first);
  let middle = tabs.open();
  let order: Vec<_> = tabs.tabs().iter().map(|tab| tab.id()).collect();
  assert_eq!(order, [first, middle, last]);
  assert_eq!(tabs.active().id(), middle);
}

#[test]
fn closing_follows_the_design() {
  let mut tabs = TabSet::default();
  let a = tabs.active().id();
  let b = tabs.open();
  let c = tabs.open();
  tabs.select(b);
  assert!(tabs.close(b));
  assert_eq!(tabs.active().id(), c, "the next tab to the right becomes active");
  assert!(tabs.close(c));
  assert_eq!(tabs.active().id(), a, "the left one when the closed tab was the last");
  let d = tabs.open();
  assert!(tabs.close(a));
  assert_eq!(tabs.active().id(), d, "closing an inactive tab keeps the active one");
  tabs.go(project(ProjectPage::Usage));
  assert!(tabs.can_close(d));
  assert!(tabs.close(d), "the only tab closes when it is not at All projects");
  assert_eq!(tabs.tabs().len(), 1);
  assert_ne!(tabs.active().id(), d, "a fresh tab replaces it");
  assert_eq!(*tabs.active().route(), Route::AllProjects);
  assert!(!tabs.active().can_back());
}

#[test]
fn cycling_wraps_around() {
  let mut tabs = TabSet::default();
  assert!(!tabs.cycle(1), "one tab has nowhere to go");
  let a = tabs.active().id();
  let b = tabs.open();
  assert!(tabs.cycle(1));
  assert_eq!(tabs.active().id(), a);
  assert!(tabs.cycle(-1));
  assert_eq!(tabs.active().id(), b);
}

#[test]
fn the_window_shows_the_active_tabs_route() {
  let mut window = Window::open();
  let navigation = window.navigation();
  assert_eq!(
    window.texts("screen-placeholder"),
    Some(vec![
      "All projects".to_owned(),
      "This screen isn't built yet.".to_owned()
    ])
  );
  assert!(window.texts("nav-overview").is_none(), "no project, no project screens");
  navigation.go(task(TaskTab::Changes));
  window.pass();
  assert_eq!(
    window.texts("screen-placeholder"),
    Some(vec![
      "KON-12 · Changes".to_owned(),
      "This screen isn't built yet.".to_owned()
    ])
  );
  assert_eq!(window.texts("tab-1"), Some(vec!["kontur · KON-12".to_owned()]));
  assert_eq!(
    window.texts("project-switcher"),
    Some(vec!["K".to_owned(), "kontur".to_owned()])
  );
  assert_eq!(window.texts("nav-tasks"), Some(vec!["Tasks".to_owned()]));
  navigation.open_tab();
  window.pass();
  assert_eq!(
    window.texts("screen-placeholder").map(|texts| texts[0].clone()),
    Some("All projects".to_owned())
  );
  navigation.cycle(-1);
  navigation.back();
  window.pass();
  assert_eq!(navigation.route_now(), Route::AllProjects);
  navigation.forward();
  window.pass();
  assert_eq!(navigation.route_now(), task(TaskTab::Changes));
  assert_eq!(window.texts("tab-1"), Some(vec!["kontur · KON-12".to_owned()]));
}

/// Presses and releases `button` over the middle of the element `id`.
fn press(window: &mut Window, id: &str, button: MouseButton) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, button);
  window.tree.mouse_up(x, y, button);
  window.pass();
}

#[test]
fn the_tab_strip_opens_switches_and_closes_tabs_by_pointer() {
  let mut window = Window::open();
  let navigation = window.navigation();
  navigation.go(project(ProjectPage::Secrets));
  window.pass();
  press(&mut window, "new-tab", MouseButton::Left);
  assert_eq!(navigation.tabs_now().tabs().len(), 2);
  assert_eq!(navigation.route_now(), Route::AllProjects, "the new tab is active");
  press(&mut window, "tab-1", MouseButton::Left);
  assert_eq!(navigation.route_now(), project(ProjectPage::Secrets));
  press(&mut window, "tab-2", MouseButton::Middle);
  assert_eq!(navigation.tabs_now().tabs().len(), 1, "a middle click closes a tab");
  press(&mut window, "tab-1-close", MouseButton::Left);
  let tabs = navigation.tabs_now();
  assert_eq!((tabs.tabs().len(), tabs.active().route()), (1, &Route::AllProjects));
  assert!(
    window.texts("tab-1-close").is_none(),
    "a fresh tab replaced the closed one"
  );
}

#[test]
fn shortcuts_ask_for_history_and_tab_moves() {
  let keys = |key: &'static str, code: &'static str| Keys {
    key,
    code,
    ..Keys::default()
  };
  let ctrl_tab = Keys {
    ctrl: true,
    ..keys("Tab", "Tab")
  };
  assert_eq!(shortcut(ctrl_tab), Some(Shortcut::NextTab));
  assert_eq!(
    shortcut(Keys {
      shift: true,
      ..ctrl_tab
    }),
    Some(Shortcut::PreviousTab)
  );
  assert_eq!(shortcut(keys("t", "KeyT")), None, "a plain key is typing");
  platform_shortcuts(keys);
}

#[cfg(not(target_os = "macos"))]
fn platform_shortcuts(keys: impl Fn(&'static str, &'static str) -> Keys<'static>) {
  let ctrl = |key, code| Keys {
    ctrl: true,
    ..keys(key, code)
  };
  let alt = |key, code| Keys {
    alt: true,
    ..keys(key, code)
  };
  assert_eq!(shortcut(ctrl("t", "KeyT")), Some(Shortcut::NewTab));
  assert_eq!(shortcut(ctrl("w", "KeyW")), Some(Shortcut::CloseTab));
  assert_eq!(shortcut(alt("ArrowLeft", "ArrowLeft")), Some(Shortcut::Back));
  assert_eq!(shortcut(alt("ArrowRight", "ArrowRight")), Some(Shortcut::Forward));
  let typing = Keys {
    typing: true,
    ..alt("ArrowLeft", "ArrowLeft")
  };
  assert_eq!(shortcut(typing), None, "a text field moves by words with Alt+Left");
}

#[cfg(target_os = "macos")]
fn platform_shortcuts(keys: impl Fn(&'static str, &'static str) -> Keys<'static>) {
  let cmd = |key, code| Keys {
    meta: true,
    ..keys(key, code)
  };
  assert_eq!(shortcut(cmd("t", "KeyT")), Some(Shortcut::NewTab));
  assert_eq!(shortcut(cmd("w", "KeyW")), Some(Shortcut::CloseTab));
  assert_eq!(shortcut(cmd("[", "BracketLeft")), Some(Shortcut::Back));
  assert_eq!(shortcut(cmd("]", "BracketRight")), Some(Shortcut::Forward));
}
