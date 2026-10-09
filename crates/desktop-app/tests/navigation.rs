//! Tabs and history: each tab keeps its own Back/Forward history, new tabs open All projects, closing
//! follows the design's rules, the window shows the active tab's route, and the shortcuts ask for the
//! right moves.

mod support;

use lurq::app::events::{MouseButton, ScrollPhase};
use orchester_app::{
  navigation::{
    shortcuts::{Keys, Shortcut, shortcut},
    tabs::{HISTORY_LIMIT, TabSet},
  },
  routes::{OverviewTab, ProjectPage, Route, TaskList, TaskTab, TaskView},
};
use support::Window;

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";

fn project(page: ProjectPage) -> Route {
  Route::Project(PROJECT.parse().expect("a project id"), page)
}

fn task(tab: TaskTab) -> Route {
  project(ProjectPage::Task(
    "KON-12".parse().expect("a task key"),
    TaskView::of(tab),
  ))
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
  assert!(tabs.go(project(ProjectPage::Overview(OverviewTab::Running))));
  assert!(tabs.go(project(ProjectPage::Tasks(TaskList::default()))));
  assert!(
    !tabs.go(project(ProjectPage::Tasks(TaskList::default()))),
    "the tab is there already"
  );
  assert!(tabs.back());
  assert_eq!(
    *tabs.active().route(),
    project(ProjectPage::Overview(OverviewTab::Running))
  );
  assert!(tabs.back());
  assert_eq!(*tabs.active().route(), Route::AllProjects);
  assert!(!tabs.back());
  assert!(tabs.forward());
  assert!(tabs.go(Route::Settings), "going from the middle drops what was ahead");
  assert!(!tabs.active().can_forward());
  assert!(tabs.back());
  assert_eq!(
    *tabs.active().route(),
    project(ProjectPage::Overview(OverviewTab::Running))
  );
}

fn searched(text: &str) -> Route {
  project(ProjectPage::Tasks(TaskList::default().with_search(text)))
}

#[test]
fn replacing_takes_the_current_place_without_a_history_entry_and_keeps_the_places_ahead() {
  let mut tabs = TabSet::default();
  tabs.go(project(ProjectPage::Overview(OverviewTab::Running)));
  tabs.go(searched("r"));
  tabs.go(project(ProjectPage::Usage));
  assert!(tabs.back());
  assert!(tabs.replace(searched("re")));
  assert!(tabs.replace(searched("retry")));
  assert!(!tabs.replace(searched("retry")), "the tab is there already");
  assert_eq!(*tabs.active().route(), searched("retry"));
  assert!(tabs.forward(), "the places ahead stay");
  assert_eq!(*tabs.active().route(), project(ProjectPage::Usage));
  assert!(tabs.back());
  assert!(tabs.back());
  assert_eq!(
    *tabs.active().route(),
    project(ProjectPage::Overview(OverviewTab::Running)),
    "the replaced places left no entries"
  );
}

#[test]
fn the_window_shows_a_replaced_route_and_back_leaves_the_screen() {
  let mut window = Window::open();
  let navigation = window.navigation();
  navigation.go(project(ProjectPage::Tasks(TaskList::default())));
  navigation.replace(searched("retry jitter"));
  window.until("the project to open", |window| window.texts("screen-tasks").is_some());
  assert_eq!(
    navigation.route_now().path(),
    format!("/project/{PROJECT}/tasks?q=retry%20jitter")
  );
  navigation.back();
  window.pass();
  assert_eq!(navigation.route_now(), Route::AllProjects);
  assert!(window.texts("screen-all_projects").is_some());
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
    window.texts("screen-all_projects"),
    Some(vec![
      "All projects".to_owned(),
      "This screen isn't built yet.".to_owned()
    ])
  );
  assert!(window.texts("nav-overview").is_none(), "no project, no project screens");
  navigation.go(task(TaskTab::Changes));
  window.until("the project to open", |window| window.texts("screen-task").is_some());
  assert_eq!(
    window.texts("screen-task"),
    Some(vec![
      "KON-12 · Changes".to_owned(),
      "This screen isn't built yet.".to_owned()
    ])
  );
  assert_eq!(window.texts("tab-1"), Some(vec!["Kontur · KON-12".to_owned()]));
  let switcher = window.texts("project-switcher").expect("the switcher shows");
  assert!(
    !switcher.iter().any(|text| text.contains(PROJECT)),
    "a project's id is never shown"
  );
  let tasks = window.texts("nav-tasks").expect("Tasks shows");
  assert_eq!(tasks[0], "Tasks");
  navigation.open_tab();
  window.pass();
  assert_eq!(
    window.texts("screen-all_projects").map(|texts| texts[0].clone()),
    Some("All projects".to_owned())
  );
  navigation.cycle(-1);
  navigation.back();
  window.pass();
  assert_eq!(navigation.route_now(), Route::AllProjects);
  navigation.forward();
  window.pass();
  assert_eq!(navigation.route_now(), task(TaskTab::Changes));
  assert_eq!(window.texts("tab-1"), Some(vec!["Kontur · KON-12".to_owned()]));
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
  let ctrl_z_on_azerty = Keys {
    ctrl: true,
    ..keys("z", "KeyW")
  };
  assert_eq!(
    shortcut(ctrl_z_on_azerty),
    None,
    "the typed letter decides, not its place"
  );
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
  assert_eq!(shortcut(ctrl("z", "KeyW")), None, "Ctrl+Z on AZERTY is not Ctrl+W");
  assert_eq!(shortcut(ctrl(",", "KeyW")), None, "Ctrl+, on Dvorak is not Ctrl+W");
  assert_eq!(shortcut(alt("[", "ArrowLeft")), None, "a typed ASCII key decides");
  assert_eq!(
    shortcut(ctrl("ц", "KeyW")),
    Some(Shortcut::CloseTab),
    "a Cyrillic layout uses the place"
  );
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
  assert_eq!(shortcut(cmd("z", "KeyW")), None, "Cmd+Z on AZERTY is not Cmd+W");
  assert_eq!(shortcut(cmd(",", "KeyW")), None, "Cmd+, on Dvorak is not Cmd+W");
  assert_eq!(
    shortcut(cmd("/", "BracketLeft")),
    None,
    "Cmd+/ on Dvorak does not go back"
  );
  assert_eq!(shortcut(cmd("[", "BracketLeft")), Some(Shortcut::Back));
  assert_eq!(shortcut(cmd("]", "BracketRight")), Some(Shortcut::Forward));
}

#[test]
fn a_tab_remembers_its_last_places_only() {
  let mut tabs = TabSet::default();
  for number in 1..=HISTORY_LIMIT + 10 {
    tabs.go(task_number(number));
  }
  let mut steps = 0;
  while tabs.back() {
    steps += 1;
  }
  assert_eq!(steps, HISTORY_LIMIT - 1);
  assert_eq!(
    *tabs.active().route(),
    task_number(11),
    "the oldest places were forgotten"
  );
}

fn task_number(number: usize) -> Route {
  project(ProjectPage::Task(
    format!("KON-{number}").parse().expect("a task key"),
    TaskView::Conversation,
  ))
}

/// The bounds of the element `id`.
fn bounds(window: &mut Window, id: &str) -> lurq::core::ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

/// A window 960 wide with 12 labelled tabs after the first, at All projects: more than fit.
fn crowded_window() -> (Window, orchester_app::navigation::Navigation) {
  let mut window = Window::open();
  window.tree.resize(960, 640);
  let navigation = window.navigation();
  for screen in [
    ProjectPage::Overview(OverviewTab::Running),
    ProjectPage::Tasks(TaskList::default()),
    ProjectPage::Pipelines,
    ProjectPage::Secrets,
  ] {
    for project in [
      PROJECT,
      "0f1e2d3c-4b5a-4968-8776-655443322110",
      "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
    ] {
      navigation.open_tab();
      navigation.go(Route::Project(project.parse().expect("a project id"), screen.clone()));
    }
  }
  window.pass();
  window.pass();
  (window, navigation)
}

/// Where the window controls begin: the laid-out minimize control on Windows. lurq's macOS traffic
/// lights carry no ids; there they take the row's last 8 and three 20.
fn controls_left(window: &mut Window) -> f32 {
  if cfg!(target_os = "macos") {
    960.0 - 68.0
  } else {
    bounds(window, "window-minimize").x
  }
}

#[test]
fn many_tabs_leave_new_tab_and_drag_area_before_the_window_controls() {
  let (mut window, _navigation) = crowded_window();
  let (new_tab, drag) = (bounds(&mut window, "new-tab"), bounds(&mut window, "title-bar-drag"));
  let controls = controls_left(&mut window);
  assert!(drag.width >= 48.0 - 0.5, "the drag area keeps 48: {drag:?}");
  assert!(
    drag.x + drag.width <= controls + 0.5,
    "the drag area ends before the controls at {controls}: {drag:?}"
  );
  assert!(
    new_tab.x + new_tab.width <= drag.x + 0.5,
    "New tab stays visible: {new_tab:?}"
  );
}

/// Whether the tab `id` shows whole in the strip's visible part.
fn in_view(window: &mut Window, id: &str) -> bool {
  let (tab, strip) = (bounds(window, id), bounds(window, "tab-scroll"));
  tab.x >= strip.x - 0.5 && tab.x + tab.width <= strip.x + strip.width + 0.5
}

fn active_id(navigation: &orchester_app::navigation::Navigation) -> String {
  format!("tab-{}", navigation.tabs_now().active().id())
}

/// Turns the mouse wheel over the strip: up (`delta` above 0) scrolls it toward its start.
fn wheel(window: &mut Window, delta: f32) {
  let strip = bounds(window, "tab-scroll");
  let (x, y) = (strip.x + strip.width / 2.0, strip.y + strip.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.scroll(x, y, 0.0, delta, ScrollPhase::Scroll);
  window.pass();
  window.pass();
}

fn settle(window: &mut Window) {
  window.pass();
  window.pass();
}

#[test]
fn the_active_tab_is_scrolled_into_view() {
  let (mut window, navigation) = crowded_window();
  let last = active_id(&navigation);
  assert!(in_view(&mut window, &last), "the newest tab shows");
  navigation.cycle(1);
  settle(&mut window);
  assert!(
    in_view(&mut window, "tab-1"),
    "going round to the first tab scrolls back to it"
  );
  navigation.cycle(-1);
  settle(&mut window);
  assert!(in_view(&mut window, &last), "and to the last again");
}

/// The ids of the open tabs, in order.
fn tab_ids(navigation: &orchester_app::navigation::Navigation) -> Vec<String> {
  navigation
    .tabs_now()
    .tabs()
    .iter()
    .map(|tab| format!("tab-{}", tab.id()))
    .collect()
}

#[test]
fn a_new_tab_at_the_right_edge_that_grows_stays_in_view() {
  let (mut window, navigation) = crowded_window();
  wheel(&mut window, 5000.0);
  let strip = bounds(&mut window, "tab-scroll");
  let rightmost = tab_ids(&navigation)
    .into_iter()
    .filter(|id| {
      let tab = bounds(&mut window, id);
      tab.x + tab.width <= strip.x + strip.width + 0.5
    })
    .last()
    .expect("a tab shows at the strip's start");
  press(&mut window, &rightmost, MouseButton::Left);
  settle(&mut window);
  navigation.open_tab();
  settle(&mut window);
  assert!(
    in_view(&mut window, &active_id(&navigation)),
    "the new All projects tab shows at the right edge"
  );
  navigation.go(project(ProjectPage::Instructions));
  settle(&mut window);
  assert!(
    in_view(&mut window, &active_id(&navigation)),
    "and still shows once it is a labelled tab"
  );
}

#[test]
fn moving_in_a_tab_the_wheel_scrolled_away_brings_it_back() {
  let (mut window, navigation) = crowded_window();
  let active = active_id(&navigation);
  wheel(&mut window, 5000.0);
  assert!(!in_view(&mut window, &active), "the wheel scrolled the active tab away");
  navigation.go(project(ProjectPage::Skills));
  settle(&mut window);
  assert!(in_view(&mut window, &active), "going somewhere in it brings it back");
  wheel(&mut window, 5000.0);
  navigation.back();
  settle(&mut window);
  assert!(in_view(&mut window, &active), "so does going back");
}

#[test]
fn a_new_tab_shows_when_the_strip_was_scrolled_to_its_end() {
  let (mut window, navigation) = crowded_window();
  // One more All projects tab at the end leaves the tab cut off on the left at the strip's end with
  // less showing than a new tab is wide, as people see it.
  navigation.open_tab();
  settle(&mut window);
  wheel(&mut window, -5000.0);
  let strip = bounds(&mut window, "tab-scroll");
  let cut = tab_ids(&navigation)
    .into_iter()
    .find(|id| {
      let tab = bounds(&mut window, id);
      tab.x < strip.x && tab.x + tab.width > strip.x
    })
    .expect("a tab is cut off on the left at the strip's end");
  navigation.cycle(-1);
  while active_id(&navigation) != cut {
    navigation.cycle(-1);
  }
  settle(&mut window);
  wheel(&mut window, -5000.0);
  assert!(
    bounds(&mut window, &cut).x < strip.x,
    "the active tab is cut off on the left"
  );
  navigation.open_tab();
  settle(&mut window);
  assert!(in_view(&mut window, &active_id(&navigation)), "the new tab shows whole");
}

#[test]
fn side_buttons_and_keys_move_through_history_and_tabs() {
  let mut window = Window::open();
  let navigation = window.navigation();
  navigation.go(project(ProjectPage::Usage));
  window.pass();
  window.tree.mouse_up(700.0, 400.0, MouseButton::Other(4));
  assert_eq!(navigation.route_now(), Route::AllProjects, "the back button goes back");
  window.tree.mouse_up(700.0, 400.0, MouseButton::Other(5));
  assert_eq!(
    navigation.route_now(),
    project(ProjectPage::Usage),
    "the forward button goes forward"
  );
  key(&mut window, "ArrowLeft", "ArrowLeft", &Modifier::Alt);
  assert_eq!(navigation.route_now(), Route::AllProjects);
  key(&mut window, "t", "KeyT", &Modifier::Command);
  assert_eq!(navigation.tabs_now().tabs().len(), 2, "a new tab opened");
  key(&mut window, "w", "KeyW", &Modifier::Command);
  assert_eq!(navigation.tabs_now().tabs().len(), 1, "the new tab closed");
}

enum Modifier {
  Alt,
  /// Ctrl, Cmd on macOS.
  Command,
}

/// Presses `key` with `modifier`, as the platform names its shortcuts.
fn key(window: &mut Window, key: &str, code: &str, modifier: &Modifier) {
  let (ctrl, alt, meta) = match modifier {
    Modifier::Alt if cfg!(target_os = "macos") => (false, false, true),
    Modifier::Alt => (false, true, false),
    Modifier::Command if cfg!(target_os = "macos") => (false, false, true),
    Modifier::Command => (true, false, false),
  };
  let (key, code) = if cfg!(target_os = "macos") && code == "ArrowLeft" {
    ("[", "BracketLeft")
  } else {
    (key, code)
  };
  window
    .tree
    .key_down_with_meta(key.to_owned(), code.to_owned(), false, ctrl, alt, meta);
  window
    .tree
    .key_up_with_meta(key.to_owned(), code.to_owned(), false, ctrl, alt, meta);
  window.pass();
}
