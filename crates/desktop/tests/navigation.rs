//! Routes: every page has a path the route table resolves and that reads back as the same page;
//! paths that name nothing lead to the nearest place that exists; the start screen, a created
//! project and the way back to the start take the history entries they should.

use std::{fs, path::Path, sync::Arc};

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx},
  components::Column,
  node::Element,
  router::RouterHandle,
};
use orchester_core::id::{ConnectionId, ProjectId, ResourceId, TaskId};
use orchester_desktop::{
  navigation::{
    Navigation, Places, Start, forward,
    route::{AgentSelection, ConnectionTab, Page, Route, SettingsTab, TaskPageTab},
    routes,
  },
  project_setup::ProjectDraft,
  session::AppSession,
  tasks::{list::TaskTab, overview::OverviewTab},
  workspace::OpenProject,
};
use parking_lot::Mutex;
use tempfile::TempDir;

/// Creates the router over the app's routes, as the root does, and hands it out.
#[derive(Clone)]
struct Probe {
  places: Places,
  router: Arc<Mutex<Option<RouterHandle>>>,
}

impl PartialEq for Probe {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.router, &other.router)
  }
}

impl lurq::app::component::DevtoolsInspectable for Probe {}

struct ProbeRoot;

impl Component for ProbeRoot {
  type Props = Probe;

  fn create(ctx: &mut Ctx) -> Self {
    let probe = ctx.props::<Probe>().clone();
    *probe.router.lock() = Some(ctx.router(routes(&probe.places)));
    Self
  }

  fn render(&self, _ctx: &mut Ctx) -> impl Into<Element> {
    Column::new()
  }
}

struct Window {
  navigation: Navigation,
  router: RouterHandle,
  session: AppSession,
  _tree: Tree,
  dir: TempDir,
}

impl Window {
  fn new() -> Self {
    let dir = TempDir::new().expect("temporary directory");
    let session = AppSession::start_at(&dir.path().join("orchester.db")).expect("session starts");
    let places = Places::default();
    let probe = Probe {
      places: places.clone(),
      router: Arc::new(Mutex::new(None)),
    };
    let mut app = App::new();
    let mut tree = Tree::new();
    tree.mount_root::<ProbeRoot>(&mut app, probe.clone());
    let router = probe.router.lock().clone().expect("the probe created the router");
    let navigation = Navigation::new(session.workspace().clone(), router.clone(), places);
    Self {
      navigation,
      router,
      session,
      _tree: tree,
      dir,
    }
  }

  fn project(&self, name: &str) -> OpenProject {
    let repo = self.dir.path().join(name);
    fs::create_dir_all(repo.join(".git")).expect("create the repository");
    let draft = ProjectDraft {
      name,
      repository: Some(Path::new(&repo)),
    };
    self
      .session
      .workspace()
      .create(draft.validate().expect("valid draft"))
      .expect("project is created")
  }

  /// A window that started on a project.
  fn on_project() -> (Self, ProjectId) {
    let window = Self::new();
    let open = window.project("kontur");
    let id = open.project.id();
    window.navigation.arrive(Start::Project(open));
    (window, id)
  }

  fn path(&self) -> String {
    self.router.path().get_untracked()
  }
}

fn pages() -> Vec<Page> {
  vec![
    Page::OVERVIEW,
    Page::Overview(OverviewTab::NeedsYou),
    Page::TASKS,
    Page::Tasks(TaskTab::Drafts),
    Page::NewTask,
    Page::task(TaskId::generate()),
    Page::Task(TaskId::generate(), TaskPageTab::Activity),
    Page::PullRequests,
    Page::PullRequest(42),
    Page::Pipelines,
    Page::Pipeline(ResourceId::generate()),
    Page::Agents(AgentSelection::First),
    Page::Agents(AgentSelection::New),
    Page::Agents(AgentSelection::Profile(ResourceId::generate())),
    Page::CONNECTIONS,
    Page::Connections(ConnectionTab::GitHub, None),
    Page::Connections(ConnectionTab::AgentProviders, Some(ConnectionId::generate())),
    Page::Settings(SettingsTab::General),
    Page::Settings(SettingsTab::Limits),
  ]
}

#[test]
fn route_every_page_resolves_and_reads_back() {
  let project = ProjectId::generate();
  let table = routes(&Places::default());
  let outside = [Route::Start, Route::Welcome, Route::Setup, Route::Unavailable];
  let inside = pages().into_iter().map(|page| Route::Project(project, page));
  for route in outside.into_iter().chain(inside) {
    let path = route.path();
    assert_eq!(Route::parse(&path), Some(route), "{path} reads back");
    let matched = table.resolve(&path);
    let leaf = matched.last().unwrap_or_else(|| panic!("{path} resolves"));
    assert_ne!(leaf.pattern_raw(), "**", "{path} is not the fallback");
  }
}

#[test]
fn route_params_hold_the_ids_of_the_path() {
  let (project, task) = (ProjectId::generate(), TaskId::generate());
  let path = Route::Project(project, Page::task(task)).path();
  let matched = routes(&Places::default()).resolve(&path);
  let params = matched.last().expect("the task route").params();
  assert_eq!(params.get_parsed::<ProjectId>("project"), Some(project));
  assert_eq!(params.get_parsed::<TaskId>("task"), Some(task));
}

#[test]
fn route_defaults_are_left_out_of_the_path() {
  let project = ProjectId::generate();
  let agents = Route::Project(project, Page::Agents(AgentSelection::First)).path();
  assert_eq!(agents, format!("/project/{project}/agents"));
  let settings = Route::Project(project, Page::Settings(SettingsTab::General)).path();
  assert_eq!(settings, format!("/project/{project}/settings"));
  assert_eq!(
    Route::Project(project, Page::CONNECTIONS).path(),
    format!("/project/{project}/connections")
  );
  assert_eq!(
    Route::Project(project, Page::OVERVIEW).path(),
    format!("/project/{project}")
  );
  assert_eq!(
    Route::Project(project, Page::TASKS).path(),
    format!("/project/{project}/tasks")
  );
  let task = TaskId::generate();
  assert_eq!(
    Route::Project(project, Page::task(task)).path(),
    format!("/project/{project}/tasks/{task}")
  );
}

#[test]
fn route_tabs_are_part_of_the_path() {
  let project = ProjectId::generate();
  let drafts = Route::Project(project, Page::Tasks(TaskTab::Drafts)).path();
  assert_eq!(drafts, format!("/project/{project}/tasks?tab=drafts"));
  // A tab that cannot be opened yet, or does not exist, is the first tab.
  let queue = format!("/project/{project}/tasks?tab=queue");
  assert_eq!(Route::parse(&queue), Some(Route::Project(project, Page::TASKS)));
  let unknown = format!("/project/{project}?tab=nothing");
  assert_eq!(Route::parse(&unknown), Some(Route::Project(project, Page::OVERVIEW)));
}

#[test]
fn route_start_is_replaced_by_the_start_screen() {
  let (window, project) = Window::on_project();
  assert_eq!(window.path(), Route::Project(project, Page::OVERVIEW).path());
  assert!(!window.router.can_back(), "the empty start route left no entry");
  // Going to the start again leads to the start screen.
  window.navigation.back_to_start();
  assert_eq!(window.path(), Route::Project(project, Page::OVERVIEW).path());
}

#[test]
fn route_invalid_ids_fall_back_to_their_section() {
  let (window, project) = Window::on_project();
  let tasks = Route::Project(project, Page::TASKS).path();
  window.router.push(format!("{tasks}/not-a-task"));
  assert_eq!(window.path(), tasks);
  let settings = Route::Project(project, Page::Settings(SettingsTab::General)).path();
  window.router.push(format!("{settings}/nothing"));
  assert_eq!(window.path(), settings);
  let pull_requests = Route::Project(project, Page::PullRequests).path();
  window.router.push(format!("{pull_requests}/-1"));
  assert_eq!(window.path(), pull_requests);
}

#[test]
fn route_unknown_places_lead_to_the_start_screen() {
  let (window, project) = Window::on_project();
  let overview = Route::Project(project, Page::OVERVIEW).path();
  window.router.push(Route::Project(project, Page::TASKS).path());
  window
    .router
    .push(Route::Project(ProjectId::generate(), Page::TASKS).path());
  assert_eq!(window.path(), overview, "a project not opened in this session");
  window.router.push("/nowhere");
  assert_eq!(window.path(), overview);
  window.router.push(Route::Unavailable.path());
  assert_eq!(window.path(), overview, "no project was reported unavailable");
}

/// The entries of the history from the first, as going back and then forward visits them.
fn entries(router: &RouterHandle) -> Vec<String> {
  while router.back() {}
  let mut paths = vec![router.path().get_untracked()];
  while forward(router) {
    paths.push(router.path().get_untracked());
  }
  paths
}

#[test]
fn route_created_project_takes_the_place_of_setup() {
  let window = Window::new();
  window.navigation.arrive(Start::Welcome);
  window.navigation.show_setup();
  assert_eq!(window.path(), Route::Setup.path());
  let created = window.project("kontur");
  let overview = Route::Project(created.project.id(), Page::OVERVIEW).path();
  window.navigation.open_project(created);
  assert_eq!(window.path(), overview);
  // The first-launch screen no longer applies, and setup was submitted: the project is the only
  // entry, and going forward does not reopen setup.
  assert!(!window.router.can_back(), "the welcome entry shows the project");
  assert!(!forward(&window.router), "setup is not visited again");
  assert_eq!(window.path(), overview);
  assert!(!window.navigation.places().applies(Route::Welcome));
  window.router.push(Route::Setup.path());
  window.router.push(Route::Welcome.path());
  assert_eq!(window.path(), overview);
}

#[test]
fn route_project_created_from_a_project_follows_it() {
  let (window, first) = Window::on_project();
  window.navigation.show_setup();
  let created = window.project("second");
  let second = Route::Project(created.project.id(), Page::OVERVIEW).path();
  window.navigation.open_project(created);
  assert_eq!(window.path(), second);
  let first = Route::Project(first, Page::OVERVIEW).path();
  assert_eq!(entries(&window.router), [first, second]);
}
