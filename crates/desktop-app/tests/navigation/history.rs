//! Every tab, filter, item and panel a person chooses is an entry of the tab's history, as in a browser:
//! Back returns to the one before and Forward comes back. Only what chooses no place takes the current
//! entry's place, and a closed panel leaves no entry. On the app's Settings, which belong to no project,
//! the sidebar keeps the project the tab was in last.

use orchester_app::{
  navigation::tabs::TabSet,
  routes::{
    AccountsView, ConnectionsView, OrchesterView, OverviewTab, ProjectPage, Route, Screen, TaskList, TaskListTab,
    TaskTab, TaskView,
  },
};
use orchester_core::{
  id::{ConnectionId, IdempotencyKey},
  name::Name,
  task::{TaskDetails, TaskLimits},
};
use orchester_workspace::{
  connections::mcp::{McpServerText, save_in},
  tasks::data::{SaveTarget, save_draft_in},
};

use super::{PROJECT, project, support::Window};

fn overview() -> Route {
  project(ProjectPage::Overview(OverviewTab::default()))
}

fn back(window: &mut Window) {
  window.navigation().back();
  window.pass();
}

fn forward(window: &mut Window) {
  window.navigation().forward();
  window.pass();
}

/// The route the active tab shows.
fn now(window: &Window) -> Route {
  window.navigation().route_now()
}

#[test]
fn a_tab_on_the_apps_settings_keeps_the_project_it_was_in_last() {
  let mut tabs = TabSet::default();
  assert_eq!(tabs.active().sidebar_project(), None, "All projects names no project");
  tabs.go(Route::Settings);
  assert_eq!(
    tabs.active().sidebar_project(),
    None,
    "a tab never in a project has none"
  );
  tabs.go(overview());
  let kontur = PROJECT.parse().expect("a project id");
  assert_eq!(tabs.active().sidebar_project(), Some(kontur));
  for settings in [
    Route::Settings,
    Route::Accounts(AccountsView::default()),
    Route::Orchester(OrchesterView::default()),
  ] {
    tabs.go(settings);
    assert_eq!(tabs.active().sidebar_project(), Some(kontur), "every tab of Settings");
  }
  tabs.go(Route::AllProjects);
  assert_eq!(
    tabs.active().sidebar_project(),
    None,
    "All projects shows no sidebar project"
  );
  tabs.back();
  assert_eq!(tabs.active().sidebar_project(), Some(kontur), "Back keeps it too");
  tabs.open();
  tabs.go(Route::Settings);
  assert_eq!(
    tabs.active().sidebar_project(),
    None,
    "a new tab has been in no project"
  );
}

#[test]
fn closing_a_panel_drops_its_entry_when_the_tab_came_from_where_it_returns() {
  let mut tabs = TabSet::default();
  let list = Route::Accounts(AccountsView::default());
  let adding = Route::Accounts(AccountsView {
    account: None,
    panel: orchester_app::routes::AccountsPanel::Add,
  });
  tabs.go(list.clone());
  tabs.go(adding.clone());
  assert!(tabs.close_panel(list.clone()));
  assert_eq!(*tabs.active().route(), list);
  assert!(!tabs.active().can_forward(), "Forward does not reopen the closed panel");
  assert!(tabs.back());
  assert_eq!(*tabs.active().route(), Route::AllProjects, "no dead entry behind it");
  tabs.forward();
  tabs.go(Route::Settings);
  tabs.go(adding);
  assert!(
    tabs.close_panel(list.clone()),
    "opened from elsewhere, it takes the panel's place"
  );
  assert_eq!(*tabs.active().route(), list);
  assert!(tabs.back());
  assert_eq!(*tabs.active().route(), Route::Settings);
  assert!(!tabs.close_panel(Route::Settings), "the tab is there already");
}

#[test]
fn the_sidebar_on_settings_shows_the_project_the_tab_was_in_last() {
  let mut window = Window::open();
  let navigation = window.navigation();
  navigation.go(Route::Settings);
  window.until("Settings", |window| window.texts("nav-settings").is_some());
  assert!(
    window.texts("nav-overview").is_none(),
    "a tab never in a project shows the project-less sidebar"
  );
  navigation.go(overview());
  window.until("Kontur's sidebar", |window| window.texts("nav-overview").is_some());
  navigation.go(Route::Settings);
  window.pass();
  assert_eq!(now(&window).nav_screen(), Screen::Settings, "Settings is selected");
  for screen in ["overview", "tasks", "tools", "project_settings"] {
    assert!(
      window.texts(&format!("nav-{screen}")).is_some(),
      "nav-{screen} stays on Settings"
    );
  }
  window.press("settings-tab-orchester");
  assert!(matches!(now(&window), Route::Orchester(_)));
  assert!(window.texts("nav-tasks").is_some(), "every tab of Settings keeps it");
  window.press("nav-tasks");
  assert_eq!(now(&window), project(ProjectPage::Tasks(TaskList::default())));
  navigation.open_tab();
  navigation.go(Route::Settings);
  window.pass();
  assert!(window.texts("nav-overview").is_none(), "another tab keeps its own");
}

#[test]
fn all_projects_and_settings_tabs_are_history() {
  let mut window = Window::open();
  window.until("All projects' tabs", |window| {
    window.texts("all-projects-tab-usage").is_some()
  });
  window.press("all-projects-tab-usage");
  assert!(matches!(now(&window), Route::Usage(_)));
  back(&mut window);
  assert_eq!(now(&window), Route::AllProjects);
  forward(&mut window);
  assert!(matches!(now(&window), Route::Usage(_)));
  window.navigation().go(Route::Settings);
  window.until("Settings' tabs", |window| {
    window.texts("settings-tab-accounts").is_some()
  });
  window.press("settings-tab-accounts");
  assert_eq!(now(&window), Route::Accounts(AccountsView::default()));
  window.press("settings-tab-orchester");
  assert_eq!(now(&window), Route::Orchester(OrchesterView::default()));
  back(&mut window);
  assert_eq!(now(&window), Route::Accounts(AccountsView::default()));
  back(&mut window);
  assert_eq!(now(&window), Route::Settings);
  forward(&mut window);
  forward(&mut window);
  assert_eq!(now(&window), Route::Orchester(OrchesterView::default()));
}

#[test]
fn tasks_tabs_and_a_tasks_tabs_are_history() {
  let mut window = Window::open();
  let draft = TaskDetails {
    title: Name::new("Show token spend per step").expect("a title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let target = SaveTarget::New(IdempotencyKey::generate());
  save_draft_in(&window.data.repo("repo-kontur"), target, &draft).expect("the draft is saved");
  let list = |tab| project(ProjectPage::Tasks(TaskList::new(tab)));
  window.navigation().go(list(TaskListTab::default()));
  window.until("the Tasks list's tabs", |window| {
    window.texts("tasks-tab-backlog").is_some()
  });
  window.press("tasks-tab-backlog");
  assert_eq!(now(&window), list(TaskListTab::Backlog));
  back(&mut window);
  assert_eq!(now(&window), list(TaskListTab::default()));
  forward(&mut window);
  assert_eq!(now(&window), list(TaskListTab::Backlog));
  let task = |tab| {
    project(ProjectPage::Task(
      "KON-1".parse().expect("a task key"),
      TaskView::of(tab),
    ))
  };
  window.navigation().go(task(TaskTab::Summary));
  window.until("the task's tabs", |window| window.texts("task-tab-changes").is_some());
  window.press("task-tab-conversation");
  assert_eq!(now(&window), task(TaskTab::Conversation));
  window.press("task-tab-changes");
  assert_eq!(now(&window), task(TaskTab::Changes));
  back(&mut window);
  assert_eq!(now(&window), task(TaskTab::Conversation));
  back(&mut window);
  assert_eq!(now(&window), task(TaskTab::Summary));
  back(&mut window);
  assert_eq!(
    now(&window),
    list(TaskListTab::Backlog),
    "Back leaves the task for the list"
  );
  forward(&mut window);
  forward(&mut window);
  assert_eq!(now(&window), task(TaskTab::Conversation));
}

/// An MCP server `name` saved in Kontur's repository.
fn server(window: &Window, name: &str) -> ConnectionId {
  let text = McpServerText {
    name: name.into(),
    program: std::env::current_exe()
      .expect("test executable")
      .to_string_lossy()
      .into_owned(),
    ..McpServerText::new_server()
  };
  save_in(
    &window.data.repo("repo-kontur"),
    None,
    text.draft(&[], &[]).expect("a server draft"),
  )
  .expect("the server is saved")
}

#[test]
fn connections_and_opened_connections_are_history() {
  let mut window = Window::open();
  let first = server(&window, "kontur-docs");
  let second = server(&window, "kontur-search");
  let connections = |view| project(ProjectPage::Connections(view));
  window.navigation().go(connections(ConnectionsView::default()));
  let row = |id: ConnectionId| format!("connection-row-{id}-row");
  window.until("the connections", |window| window.texts(&row(second)).is_some());
  window.press(&row(second));
  let opened_second = connections(ConnectionsView::of(second));
  assert_eq!(now(&window), opened_second);
  window.until("the connection's page", |window| {
    window.texts("connection-crumb-back").is_some()
  });
  window.press("connection-crumb-back");
  assert_eq!(now(&window), connections(ConnectionsView::default()));
  window.until("the connections", |window| window.texts(&row(first)).is_some());
  window.press(&row(first));
  assert_eq!(now(&window), connections(ConnectionsView::of(first)));
  back(&mut window);
  assert_eq!(now(&window), connections(ConnectionsView::default()));
  back(&mut window);
  assert_eq!(now(&window), opened_second);
  back(&mut window);
  assert_eq!(now(&window), connections(ConnectionsView::default()));
  forward(&mut window);
  assert_eq!(now(&window), opened_second);
}
