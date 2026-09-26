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
    Navigation, Places, Start,
    route::{AgentSelection, ConnectionTab, Page, Route, SettingsTab},
    routes,
  },
  project_setup::ProjectDraft,
  session::AppSession,
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
    Page::Overview,
    Page::Tasks,
    Page::NewTask,
    Page::Task(TaskId::generate()),
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
  let path = Route::Project(project, Page::Task(task)).path();
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
}

#[test]
fn route_start_is_replaced_by_the_start_screen() {
  let (window, project) = Window::on_project();
  assert_eq!(window.path(), Route::Project(project, Page::Overview).path());
  assert!(!window.router.can_back(), "the empty start route left no entry");
  // Going to the start again leads to the start screen.
  window.navigation.back_to_start();
  assert_eq!(window.path(), Route::Project(project, Page::Overview).path());
}

#[test]
fn route_invalid_ids_fall_back_to_their_section() {
  let (window, project) = Window::on_project();
  let tasks = Route::Project(project, Page::Tasks).path();
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
  let overview = Route::Project(project, Page::Overview).path();
  window.router.push(Route::Project(project, Page::Tasks).path());
  window
    .router
    .push(Route::Project(ProjectId::generate(), Page::Tasks).path());
  assert_eq!(window.path(), overview, "a project not opened in this session");
  window.router.push("/nowhere");
  assert_eq!(window.path(), overview);
  window.router.push(Route::Unavailable.path());
  assert_eq!(window.path(), overview, "no project was reported unavailable");
}

#[test]
fn route_created_project_takes_the_place_of_setup() {
  let window = Window::new();
  window.navigation.arrive(Start::Welcome);
  window.navigation.show_setup();
  assert_eq!(window.path(), Route::Setup.path());
  let created = window.project("kontur");
  let overview = Route::Project(created.project.id(), Page::Overview).path();
  window.navigation.open_project(created);
  assert_eq!(window.path(), overview);
  assert!(window.router.back());
  assert_eq!(window.path(), Route::Welcome.path(), "setup left no entry");
  assert!(!window.router.can_back());
}
