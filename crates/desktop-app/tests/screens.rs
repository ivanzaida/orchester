//! Every route's path shows its screen's own element, `screen-<name>`: an honest placeholder until the
//! screen is built (`BUILT`; Settings, built in part, is `tests/settings.rs`). A path that names nothing shows
//! `screen-not-found`, which says why. The paths go straight to the router, as a tab's history would hand them over, so
//! a path no route prints is shown too. The project the paths name is known to be open; what the window does until a
//! project opens, and for one that cannot, is `tests/projects.rs`. The shell around the screens works with a session's
//! services, from a data folder of the test's own.

mod support;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    App, Tree,
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Router,
  node::{Element, element::ElementRef},
  router::RouterHandle,
};
use orchester_app::{
  i18n,
  mcp::{McpKind, McpLink, McpSettings, McpStatus},
  navigation::{Navigation, tabs::TabSet},
  project::{ProjectState, Projects},
  root::Start,
  routes::{
    ChangeSource, ChangedFile, ConnectionsTab, ConnectionsView, NewConnection, OverviewTab, ProjectPage, Route, Screen,
    TaskList, TaskListTab, TaskTab, TaskView, routes,
  },
  services::Services,
  startup::Launch,
  theme::{self, Mode},
};
use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_workspace::{tasks::integration_jobs::ProjectJobs, workspace::OpenProject};
use parking_lot::Mutex;
use support::Data;

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";
const CONNECTION: &str = "7e1f2a3b-4c5d-4e6f-8a9b-0c1d2e3f4a5b";
const NOT_BUILT: &str = "This screen isn't built yet.";
/// What a task's page shows while the project's tasks are read; nothing reads them here.
const READING_TASK: &str = "Reading the task…";
/// The screens that show their own content, which their own tests check, in place of the placeholder.
const BUILT: &[Screen] = &[
  // One per line, sorted, so each screen's task adds a line of its own.
  Screen::AddProject,
  Screen::Agents,
  Screen::AllProjects,
  Screen::Connections,
  Screen::EditConnection,
  Screen::EditTask,
  Screen::NewConnection,
  Screen::NewTask,
  Screen::Pipelines,
  Screen::ProjectSettings,
  Screen::Secrets,
  Screen::Skills,
  Screen::Tasks,
  Screen::Tools,
];

/// Where the test root leaves its router, and the services it provides.
#[derive(Clone)]
struct RouterSlot(Arc<Mutex<Option<RouterHandle>>>, Services);

impl PartialEq for RouterSlot {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for RouterSlot {}

/// The shell and its screens without the window's chrome, around a router the test drives.
struct ScreensRoot {
  router: RouterHandle,
  navigation: Navigation,
  projects: Projects,
}

impl Component for ScreensRoot {
  type Props = RouterSlot;

  fn create(ctx: &mut Ctx) -> Self {
    let project = PROJECT.parse().expect("a project id");
    let router = ctx.router(routes());
    let navigation = Navigation::new(ctx.signal(TabSet::default()), router.clone());
    *ctx.props::<RouterSlot>().0.lock() = Some(router.clone());
    Self {
      router,
      navigation,
      projects: Projects::new(
        Some((project, ProjectState::Open(kontur(project)))),
        ProjectJobs::default(),
      ),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    ctx.provide(ctx.props::<RouterSlot>().1.clone());
    ctx.provide(McpLink::without_server(McpStatus::unavailable(
      McpKind::Installation,
      McpSettings::of_build(),
    )));
    ctx.provide(self.navigation.clone());
    ctx.provide(self.projects.clone());
    Router::mount(ctx, self.router.clone())
  }
}

struct Screens {
  app: App,
  tree: Tree,
  router: RouterHandle,
  /// Keep the session, its data folder and its runtime while the screens show.
  _session: (Launch, Data),
}

impl Screens {
  fn open() -> Self {
    let data = Data::new();
    let launch = data.launch();
    let Start::Ready(ready) = launch.start() else {
      panic!("the session starts");
    };
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    app.set_tokio_handle(data.runtime.handle().clone());
    let slot = RouterSlot(Arc::default(), ready.services);
    tree.mount_root::<ScreensRoot>(&mut app, slot.clone());
    tree.pass_headless(&mut app);
    let router = slot.0.lock().take().expect("the root left its router");
    Self {
      app,
      tree,
      router,
      _session: (launch, data),
    }
  }

  /// The texts the screen at `path` shows under the element `id`, icon glyphs left out; `None` when
  /// no element has the id.
  fn show(&mut self, path: &str, id: &str) -> Option<Vec<String>> {
    self.router.replace(path);
    // A screen that reads its data first shows its element once the reading is delivered.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
      self.tree.tick_futures();
      self.tree.pass_headless(&mut self.app);
      if self.tree.get_element_by_id(id).is_some() || std::time::Instant::now() > deadline {
        break;
      }
      std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let element = self.tree.get_element_by_id(id)?;
    let mut all = Vec::new();
    collect(element, &mut all);
    all.retain(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)));
    Some(all)
  }
}

fn collect(element: ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    collect(child, into);
  }
}

/// Kontur as an open project; nothing reads its folder here.
fn kontur(id: ProjectId) -> OpenProject {
  OpenProject {
    project: Project::new(
      id,
      ProjectKey::new("KON").expect("a project key"),
      Name::new("Kontur").expect("a project name"),
    ),
    repo_root: PathBuf::from("kontur"),
  }
}

fn at(page: ProjectPage) -> Route {
  Route::Project(PROJECT.parse().expect("a project id"), page)
}

fn key(key: &str) -> orchester_core::project::TaskKey {
  key.parse().expect("a task key")
}

/// A route of every screen, with what it shows: a placeholder's title and that it is not built, or what
/// a built screen shows first.
fn every_screen() -> Vec<(Route, Vec<&'static str>)> {
  let changes = TaskView::Changes(Some(
    ChangedFile::new("src/a b.rs", ChangeSource::Worktree).expect("a file path"),
  ));
  let placeholder = |title| vec![title, NOT_BUILT];
  // Settings is built in part; `tests/settings.rs` shows it.
  let mut all = vec![(Route::AllProjects, Vec::new()), (Route::AddProject(None), Vec::new())];
  all.extend([
    (at(ProjectPage::Overview(OverviewTab::Failed)), placeholder("Overview")),
    (
      at(ProjectPage::Tasks(
        TaskList::new(TaskListTab::Done)
          .with_search("retry & jitter")
          .with_highlight(key("KON-13")),
      )),
      placeholder("Tasks"),
    ),
    (at(ProjectPage::NewTask), placeholder("New task")),
    (at(ProjectPage::EditTask(key("KON-7"))), placeholder("Edit KON-7")),
    (
      at(ProjectPage::Task(key("KON-12"), TaskView::of(TaskTab::Summary))),
      vec![READING_TASK],
    ),
    (at(ProjectPage::Task(key("KON-12"), changes)), vec![READING_TASK]),
    (at(ProjectPage::PullRequests), placeholder("Pull requests")),
    (at(ProjectPage::Usage), placeholder("Usage")),
    (at(ProjectPage::Pipelines), placeholder("Pipelines")),
    (at(ProjectPage::Agents), placeholder("Agents")),
    (at(ProjectPage::Instructions), placeholder("Instructions")),
    (at(ProjectPage::Skills), placeholder("Skills")),
    (at(ProjectPage::Tools), placeholder("Tools")),
    (at(ProjectPage::Secrets), placeholder("Secrets")),
    (
      at(ProjectPage::Connections(ConnectionsView::new(
        ConnectionsTab::McpServers,
      ))),
      placeholder("Connections"),
    ),
    (
      at(ProjectPage::NewConnection(NewConnection::McpServer)),
      placeholder("Add connection"),
    ),
    (
      at(ProjectPage::EditConnection(
        CONNECTION.parse().expect("a connection id"),
      )),
      placeholder("Edit connection"),
    ),
    (at(ProjectPage::Settings), placeholder("Project settings")),
  ]);
  all
}

#[test]
fn screens_each_route_mounts_its_own_placeholder() {
  let mut screens = Screens::open();
  let routes = every_screen();
  for screen in Screen::ALL {
    assert!(
      screen == Screen::Settings || routes.iter().any(|(route, _)| route.screen() == screen),
      "{screen:?} is covered"
    );
  }
  for (route, expected) in routes {
    let id = format!("screen-{}", route.screen().name());
    let texts = screens
      .show(&route.path(), &id)
      .unwrap_or_else(|| panic!("{} shows {id}", route.path()));
    if !BUILT.contains(&route.screen()) {
      assert_eq!(texts, expected, "{}", route.path());
    }
    assert!(
      screens.tree.get_element_by_id("screen-not-found").is_none(),
      "{}",
      route.path()
    );
  }
}

#[test]
fn screens_a_path_that_names_nothing_says_why() {
  let mut screens = Screens::open();
  let task = format!("/project/{PROJECT}/tasks");
  let cases = [
    ("/nope".to_owned(), "No screen has the address /nope.".to_owned()),
    (
      "/project/kontur/tasks".to_owned(),
      "kontur in /project/kontur/tasks is not a project id.".to_owned(),
    ),
    (
      format!("{task}/kon-12/edit"),
      format!("kon-12 in {task}/kon-12/edit is not a task key."),
    ),
    (
      format!("{task}/KON-12/summary?file=a.rs"),
      format!("The screen at {task}/KON-12/summary?file=a.rs does not take file as given."),
    ),
    (
      format!("{task}?tab=running"),
      format!("running is not a value of tab in {task}?tab=running."),
    ),
  ];
  for (path, reason) in cases {
    let texts = screens
      .show(&path, "screen-not-found")
      .unwrap_or_else(|| panic!("{path} shows why it names nothing"));
    assert_eq!(texts, ["Nothing here".to_owned(), reason], "{path}");
  }
}
