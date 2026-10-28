//! Tabs and history: each tab keeps its own Back/Forward history, new tabs open All projects, closing
//! follows the design's rules, the window shows the active tab's route, and the shortcuts ask for the
//! right moves.

mod history;
mod items;
mod list;
mod strip;
#[path = "../support/mod.rs"]
mod support;

use lurq::app::events::MouseButton;
use orchester_app::{
  navigation::{
    shortcuts::{Keys, Shortcut, shortcut},
    tabs::{HISTORY_LIMIT, TabSet},
  },
  routes::{OverviewTab, ProjectPage, Route, TaskList, TaskTab, TaskView},
};
use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  task::{TaskDetails, TaskLimits},
};
use orchester_workspace::tasks::data::{SaveTarget, save_draft_in};
use support::Window;

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";

fn project(page: ProjectPage) -> Route {
  Route::Project(PROJECT.parse().expect("a project id"), page)
}

/// KON-1's page on `tab`: the window test's draft.
fn draft_route(tab: TaskTab) -> Route {
  project(ProjectPage::Task(
    "KON-1".parse().expect("a task key"),
    TaskView::of(tab),
  ))
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
  tabs.go(project(ProjectPage::Usage(Box::default())));
  assert!(tabs.back());
  assert!(tabs.replace(searched("re")));
  assert!(tabs.replace(searched("retry")));
  assert!(!tabs.replace(searched("retry")), "the tab is there already");
  assert_eq!(*tabs.active().route(), searched("retry"));
  assert!(tabs.forward(), "the places ahead stay");
  assert_eq!(*tabs.active().route(), project(ProjectPage::Usage(Box::default())));
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
  tabs.go(project(ProjectPage::Usage(Box::default())));
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
  window.until("All projects to be read", |window| {
    window
      .texts("all-projects-title")
      .is_some_and(|title| title == ["All projects"])
  });
  assert!(window.texts("nav-overview").is_none(), "no project, no project screens");
  // KON-1, a draft: its Changes tab says it has not run yet.
  let draft = TaskDetails {
    title: Name::new("Show token spend per step").expect("a title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let target = SaveTarget::New(IdempotencyKey::generate());
  save_draft_in(&window.data.repo("repo-kontur"), target, &draft).expect("the draft is saved");
  let changes = draft_route(TaskTab::Changes);
  navigation.go(changes.clone());
  window.until("the task to be read", |window| window.texts("changes-no-run").is_some());
  let tab = window.texts("task-tab-changes").expect("the page tabs show");
  assert_eq!(tab, ["Changes"], "a task without a run has no count");
  let card = window.texts("changes-no-run").expect("the Changes tab shows");
  assert_eq!(card[0], "No run yet");
  assert_eq!(window.texts("tab-1"), Some(vec!["Kontur · KON-1".to_owned()]));
  let switcher = window.texts("project-switcher").expect("the switcher shows");
  assert!(
    !switcher.iter().any(|text| text.contains(PROJECT)),
    "a project's id is never shown"
  );
  let tasks = window.texts("nav-tasks").expect("Tasks shows");
  assert_eq!(tasks[0], "Tasks");
  navigation.open_tab();
  window.until("the new tab's All projects", |window| {
    window
      .texts("all-projects-title")
      .is_some_and(|title| title == ["All projects"])
  });
  navigation.cycle(-1);
  navigation.back();
  window.pass();
  assert_eq!(navigation.route_now(), Route::AllProjects);
  navigation.forward();
  window.pass();
  assert_eq!(navigation.route_now(), changes);
  assert_eq!(window.texts("tab-1"), Some(vec!["Kontur · KON-1".to_owned()]));
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
  navigation.go(project(ProjectPage::Secrets(Default::default())));
  window.pass();
  press(&mut window, "new-tab", MouseButton::Left);
  assert_eq!(navigation.tabs_now().tabs().len(), 2);
  assert_eq!(navigation.route_now(), Route::AllProjects, "the new tab is active");
  press(&mut window, "tab-1", MouseButton::Left);
  assert_eq!(
    navigation.route_now(),
    project(ProjectPage::Secrets(Default::default()))
  );
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
  assert_eq!(
    shortcut(ctrl(",", "KeyW")),
    Some(Shortcut::ProjectSettings),
    "Ctrl+, on Dvorak is Ctrl+, not Ctrl+W"
  );
  assert_eq!(shortcut(ctrl("k", "KeyK")), Some(Shortcut::JumpTo));
  assert_eq!(
    shortcut(Keys {
      typing: true,
      ..ctrl("k", "KeyK")
    }),
    Some(Shortcut::JumpTo),
    "Ctrl+K opens Jump to from a text field too"
  );
  assert_eq!(shortcut(ctrl("p", "KeyP")), None, "Files keeps Ctrl+P for Go to file");
  assert_eq!(
    shortcut(Keys {
      shift: true,
      ..ctrl("k", "KeyK")
    }),
    None,
    "Ctrl+Shift+K is no shortcut"
  );
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
  assert_eq!(
    shortcut(cmd(",", "KeyW")),
    Some(Shortcut::ProjectSettings),
    "Cmd+, on Dvorak is Cmd+, not Cmd+W"
  );
  assert_eq!(shortcut(cmd("k", "KeyK")), Some(Shortcut::JumpTo));
  assert_eq!(shortcut(cmd("p", "KeyP")), None, "Files keeps Cmd+P for Go to file");
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

#[test]
fn side_buttons_and_keys_move_through_history_and_tabs() {
  let mut window = Window::open();
  let navigation = window.navigation();
  navigation.go(project(ProjectPage::Usage(Box::default())));
  window.pass();
  window.tree.mouse_up(700.0, 400.0, MouseButton::Other(4));
  assert_eq!(navigation.route_now(), Route::AllProjects, "the back button goes back");
  window.tree.mouse_up(700.0, 400.0, MouseButton::Other(5));
  assert_eq!(
    navigation.route_now(),
    project(ProjectPage::Usage(Box::default())),
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
