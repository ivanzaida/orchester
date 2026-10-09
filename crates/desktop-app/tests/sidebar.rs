//! The sidebar with a session's data: how many tasks the project has (a dash while they are read), the
//! runs under way as the Running list shows them, the project switcher's RECENT (five at most, the shown
//! project checked), and what *Open folder…* makes of a chosen folder.

mod support;

use std::fs;

use lurq::{
  app::{
    App, Tree,
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseButton,
  },
  node::{Element, element::ElementRef},
};
use orchester_app::{
  i18n,
  navigation::{Navigation, tabs::TabSet},
  project::Projects,
  root::Start,
  routes::{OverviewTab, ProjectPage, Route, routes},
  shell::sidebar::{
    running::{RowState, RunningRow, running_list},
    switcher::{FolderRefusal, open_folder},
  },
  theme::{self, Mode},
};
use orchester_core::{id::ProjectId, run::ActiveClock, time::Timestamp};
use support::{Data, Window, add_project, project_id};

fn overview(project: ProjectId) -> Route {
  Route::Project(project, ProjectPage::Overview(OverviewTab::default()))
}

/// Presses the middle of the element `id`.
fn press(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
}

#[test]
fn sidebar_counts_the_tasks_with_a_dash_while_they_are_read() {
  let mut window = Window::open();
  let Start::Ready(ready) = window.launch.start() else {
    panic!("the session started");
  };
  ready.services.hold.set(true);
  window.navigation().go(overview(project_id()));
  window.until("the project to open", |window| {
    window.texts("nav-tasks").is_some_and(|texts| texts.len() == 2)
  });
  assert_eq!(
    window.texts("nav-tasks"),
    Some(vec!["Tasks".to_owned(), "—".to_owned()])
  );
  ready.services.hold.set(false);
  window.until("the tasks to be read", |window| {
    window.texts("nav-tasks") == Some(vec!["Tasks".to_owned(), "0".to_owned()])
  });
  assert!(
    window.texts("running").is_none(),
    "no run under way, no Running section"
  );
}

#[test]
fn sidebar_recent_lists_five_projects_with_the_shown_one_checked() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started");
  let others: Vec<ProjectId> = ["AAA", "BBB", "CCC", "DDD", "EEE"]
    .iter()
    .map(|key| {
      let id = ProjectId::generate();
      add_project(workspace, (id, key, key), &data.repo(&format!("repo-{key}")));
      id
    })
    .collect();
  data.add_kontur(workspace);
  let mut window = Window::of(data, launch);
  window.navigation().go(overview(project_id()));
  window.until("the project to open", |window| {
    window
      .texts("project-switcher")
      .is_some_and(|texts| texts.contains(&"Kontur".to_owned()))
  });
  press(&mut window, "project-switcher");
  window.until("RECENT to be read", |window| {
    window.texts(&format!("recent-{}", project_id())).is_some()
  });
  let menu = window.texts("project-switcher-menu").expect("the menu is open");
  assert_eq!(menu[0], "RECENT");
  assert!(
    window.texts(&format!("recent-{}-current", project_id())).is_some(),
    "Kontur is checked"
  );
  let listed: Vec<&ProjectId> = others
    .iter()
    .filter(|id| window.texts(&format!("recent-{id}")).is_some())
    .collect();
  assert_eq!(listed.len(), 4, "five rows: Kontur, used last, and four of the others");
  assert!(
    window.texts(&format!("recent-{}", others[0])).is_none(),
    "the project used longest ago is left out"
  );
  for id in &others[1..] {
    assert!(
      window.texts(&format!("recent-{id}-current")).is_none(),
      "{id} is not checked"
    );
  }
  for item in ["switcher-all-projects", "switcher-open-folder", "switcher-add-project"] {
    assert!(window.texts(item).is_some(), "{item} follows RECENT");
  }
  assert!(
    window
      .texts("switcher-add-project")
      .is_some_and(|texts| texts.iter().any(|text| text.starts_with("Not in this app yet"))),
    "Add project says why it is unavailable"
  );
}

#[test]
fn sidebar_open_folder_opens_a_project_folder_and_refuses_others() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started");
  let kontur = data.add_kontur(workspace);
  let opened = open_folder(workspace, &kontur.repo_root).expect("a project folder opens");
  assert_eq!(opened.project.id(), project_id());
  let plain = data.dir.path().join("plain");
  fs::create_dir_all(&plain).expect("a folder");
  assert_eq!(open_folder(workspace, &plain), Err(FolderRefusal::NotRepository));
  let inside = kontur.repo_root.join("src");
  fs::create_dir_all(&inside).expect("a folder in the repository");
  assert_eq!(
    open_folder(workspace, &inside),
    Err(FolderRefusal::NotTopFolder { top: kontur.repo_root })
  );
  let bare = data.repo("repo-bare");
  assert_eq!(
    open_folder(workspace, &bare),
    Err(FolderRefusal::NoProject),
    "a repository without .orchester"
  );
  assert_eq!(
    workspace.projects().expect("the list reads").len(),
    1,
    "nothing refused is added to the list"
  );
}

/// The Running list alone, with the rows the test gives it.
#[derive(Clone, PartialEq)]
struct Rows(Vec<RunningRow>);

impl DevtoolsInspectable for Rows {}

struct RunningRoot {
  navigation: Navigation,
}

impl Component for RunningRoot {
  type Props = Rows;

  fn create(ctx: &mut Ctx) -> Self {
    let router = ctx.router(routes());
    Self {
      navigation: Navigation::new(ctx.signal(TabSet::default()), router),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    ctx.provide(self.navigation.clone());
    ctx.provide(Projects::new(None));
    let rows = ctx.props::<Rows>().0.clone();
    running_list(ctx, project_id(), &rows)
  }
}

fn texts(tree: &mut Tree, id: &str) -> Vec<String> {
  fn collect(element: ElementRef<'_>, into: &mut Vec<String>) {
    into.extend(element.text_content().map(str::to_owned));
    for child in element.children() {
      collect(child, into);
    }
  }
  let element = tree.get_element_by_id(id).unwrap_or_else(|| panic!("{id} shows"));
  let mut all = Vec::new();
  collect(element, &mut all);
  all.retain(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)));
  all
}

#[test]
fn sidebar_running_list_names_each_run_its_step_and_time() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app).expect("bundled locales load");
  let mut tree = Tree::new();
  tree.resize(1440, 900);
  let key = |key: &str| key.parse().expect("a task key");
  let still = ActiveClock::of([], Timestamp::now());
  let rows = vec![
    RunningRow {
      key: key("KON-12"),
      state: RowState::Working,
      step: Some("Implement".into()),
      clock: Some(still),
    },
    RunningRow {
      key: key("KON-16"),
      state: RowState::NeedsYou,
      step: Some("Plan".into()),
      clock: Some(still),
    },
    RunningRow {
      key: key("KON-21"),
      state: RowState::Stopped,
      step: Some("Review".into()),
      clock: None,
    },
  ];
  tree.mount_root::<RunningRoot>(&mut app, Rows(rows));
  tree.pass_headless(&mut app);
  assert_eq!(texts(&mut tree, "running")[0], "Running");
  assert_eq!(texts(&mut tree, "running-KON-12"), ["KON-12 · Implement", "0s"]);
  assert_eq!(texts(&mut tree, "running-KON-16"), ["KON-16 · Plan", "0s"]);
  assert_eq!(
    texts(&mut tree, "running-KON-21"),
    ["KON-21 · stopped", "—"],
    "no runner looks after its agent"
  );
}
