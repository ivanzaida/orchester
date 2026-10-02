//! Routes: every page has a path the route table resolves and that reads back as the same page;
//! paths that name nothing lead to the nearest place that exists; the start screen, a created
//! project and the way back to the start take the history entries they should.

use std::{fs, num::NonZeroU32, path::Path, sync::Arc};

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx},
  components::Column,
  node::Element,
  router::RouterHandle,
};
use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId, SecretId, TaskId},
  resource::VersionNumber,
};
use orchester_desktop::{
  navigation::{
    Navigation, Places, Start, forward,
    import::{self, ImportTab},
    repository_import,
    route::{
      AgentSelection, ChangedFile, ConnectionTab, InstructionsTab, Page, PipelineView, Route, SettingsTab, TaskPageTab,
      ToolSelection, changed_file, changes_path,
    },
    routes,
  },
  project_setup::ProjectDraft,
  session::AppSession,
  tabs::{OpenIn, Tabs, model::TabSet},
  tasks::{list::TaskTab, overview::OverviewTab},
  workspace::OpenProject,
};
use parking_lot::Mutex;
use tempfile::TempDir;
use tokio::runtime::Runtime;

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
  _runtime: Runtime,
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
    let runtime = Runtime::new().expect("tokio runtime");
    let home = Route::AllProjects.path();
    let tabs = Tabs::new(
      TabSet::new(&home, &Route::Start.path()),
      places.clone(),
      session.workspace().clone(),
      runtime.handle().clone(),
    );
    let navigation = Navigation::new(session.workspace().clone(), router.clone(), places, tabs);
    Self {
      navigation,
      router,
      session,
      _tree: tree,
      _runtime: runtime,
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
    window.arrive(Start::Project(open));
    (window, id)
  }

  fn path(&self) -> String {
    self.router.path().get_untracked()
  }

  /// The start screen is known: the tab opens it, as a restored or new window does.
  fn arrive(&self, start: Start) {
    let route = self.navigation.places().set_start(start);
    self.router.replace(route.path());
  }
}

fn version(number: u32) -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(number).expect("versions start at 1"))
}

fn pages() -> Vec<Page> {
  vec![
    Page::OVERVIEW,
    Page::Overview(OverviewTab::NeedsYou),
    Page::Overview(OverviewTab::Failed),
    Page::TASKS,
    Page::Tasks(TaskTab::Drafts),
    Page::NewTask,
    Page::task(TaskId::generate()),
    Page::Task(TaskId::generate(), TaskPageTab::Conversation),
    Page::Task(TaskId::generate(), TaskPageTab::Changes),
    Page::EditTask(TaskId::generate()),
    Page::RetryTask(TaskId::generate()),
    Page::PullRequests,
    Page::PullRequest(42),
    Page::Pipelines,
    Page::Pipeline(ResourceId::generate(), PipelineView::Draft),
    Page::Pipeline(ResourceId::generate(), PipelineView::Version(version(2))),
    Page::Pipeline(ResourceId::generate(), PipelineView::Compare(version(3))),
    Page::Instructions(InstructionsTab::ProjectRules, None),
    Page::Instructions(InstructionsTab::StepOverrides, Some(ResourceId::generate())),
    Page::Skills(None),
    Page::Skills(Some(ResourceId::generate())),
    Page::Agents(AgentSelection::First),
    Page::Agents(AgentSelection::New),
    Page::Agents(AgentSelection::Profile(ResourceId::generate())),
    Page::Tools(ToolSelection::Library),
    Page::Tools(ToolSelection::New),
    Page::Tools(ToolSelection::Tool(ResourceId::generate())),
    Page::SECRETS,
    Page::Secrets(Some(SecretId::generate())),
    Page::CONNECTIONS,
    Page::Connections(ConnectionTab::GitHub, None),
    Page::Connections(ConnectionTab::AgentProviders, Some(ConnectionId::generate())),
    Page::Connections(ConnectionTab::McpServers, Some(ConnectionId::generate())),
    Page::Settings(SettingsTab::General),
    Page::Settings(SettingsTab::Limits),
    Page::Settings(SettingsTab::Environment),
    Page::Settings(SettingsTab::CodeIndex),
  ]
}

#[test]
fn route_every_page_resolves_and_reads_back() {
  let project = ProjectId::generate();
  let table = routes(&Places::default());
  let outside = [Route::Start, Route::Welcome, Route::Setup, Route::AllProjects];
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
fn route_the_activity_tab_of_earlier_paths_opens_the_conversation() {
  let (project, task) = (ProjectId::generate(), TaskId::generate());
  let conversation = Route::Project(project, Page::Task(task, TaskPageTab::Conversation));
  let earlier = conversation.path().replace("tab=conversation", "tab=activity");
  assert_ne!(earlier, conversation.path(), "the tab is in the path's query");
  assert_eq!(Route::parse(&earlier), Some(conversation));
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
  let roles = Route::Project(project, Page::Instructions(InstructionsTab::AgentRoles, None)).path();
  assert_eq!(roles, format!("/project/{project}/instructions?tab=agent-roles"));
  let rules = Route::Project(project, Page::Instructions(InstructionsTab::ProjectRules, None)).path();
  assert_eq!(rules, format!("/project/{project}/instructions"));
}

#[test]
fn route_pipeline_version_and_comparison_are_part_of_the_path() {
  let (project, pipeline) = (ProjectId::generate(), ResourceId::generate());
  let draft = Route::Project(project, Page::Pipeline(pipeline, PipelineView::Draft)).path();
  assert_eq!(draft, format!("/project/{project}/pipelines/{pipeline}"));
  let shown = Route::Project(project, Page::Pipeline(pipeline, PipelineView::Version(version(2)))).path();
  assert_eq!(shown, format!("{draft}?version=2"));
  let compared = Route::Project(project, Page::Pipeline(pipeline, PipelineView::Compare(version(2)))).path();
  assert_eq!(compared, format!("{draft}?version=2&compare=draft"));
  // Comparing with anything but the draft is not a view; the version alone is shown.
  assert_eq!(
    Route::parse(&format!("{draft}?version=2&compare=v1")),
    Some(Route::Project(
      project,
      Page::Pipeline(pipeline, PipelineView::Version(version(2)))
    ))
  );
  // A view is one screen: moving between versions keeps the editor.
  assert!(
    Page::Pipeline(pipeline, PipelineView::Draft)
      .same_screen(&Page::Pipeline(pipeline, PipelineView::Version(version(1))))
  );
  assert!(
    !Page::Pipeline(pipeline, PipelineView::Draft)
      .same_screen(&Page::Pipeline(ResourceId::generate(), PipelineView::Draft))
  );
}

#[test]
fn route_pipeline_version_that_is_not_a_number_is_the_draft() {
  let (project, pipeline) = (ProjectId::generate(), ResourceId::generate());
  let draft = Route::Project(project, Page::Pipeline(pipeline, PipelineView::Draft));
  for bad in ["?version=0", "?version=two", "?version=-1", "?compare=draft"] {
    assert_eq!(Route::parse(&format!("{}{bad}", draft.path())), Some(draft), "{bad}");
  }
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
  window.router.push(Route::Welcome.path());
  assert_eq!(
    window.path(),
    overview,
    "the first launch is over once a project is open"
  );
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
  window.arrive(Start::Welcome);
  window.navigation.open(Route::Setup, OpenIn::Current);
  assert_eq!(window.path(), Route::Setup.path());
  let created = window.project("kontur");
  let overview = Route::Project(created.project.id(), Page::OVERVIEW).path();
  window.navigation.open_created(created);
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
  window.navigation.open(Route::Setup, OpenIn::Current);
  let created = window.project("second");
  let second = Route::Project(created.project.id(), Page::OVERVIEW).path();
  window.navigation.open_created(created);
  assert_eq!(window.path(), second);
  let first = Route::Project(first, Page::OVERVIEW).path();
  assert_eq!(entries(&window.router), [first, second]);
}

#[test]
fn route_changes_tab_keeps_the_file_in_its_path() {
  let (project, task) = (ProjectId::generate(), TaskId::generate());
  for (path, worktree) in [
    ("src/auth/github.rs", false),
    ("docs/plan with space & more?#.md", true),
    ("naïve/файл.txt", false),
  ] {
    let file = ChangedFile {
      path: path.to_owned(),
      worktree,
    };
    let route = changes_path(project, task, &file);
    assert_eq!(
      Route::parse(&route),
      Some(Route::Project(project, Page::Task(task, TaskPageTab::Changes))),
      "{route}"
    );
    assert!(route.is_ascii(), "{route}");
    assert_eq!(changed_file(&route), Some(file));
    let matched = routes(&Places::default()).resolve(&route);
    assert_ne!(matched.last().map(|leaf| leaf.pattern_raw()), Some("**"), "{route}");
  }
  let tab = Route::Project(project, Page::Task(task, TaskPageTab::Changes)).path();
  assert_eq!(changed_file(&tab), None);
  assert_eq!(changed_file(&format!("{tab}&file=%zz")), None);
}

#[test]
fn route_import_opens_over_the_page_and_closes_to_it() {
  let project = ProjectId::generate();
  for page in [
    Page::Pipelines,
    Page::Tools(ToolSelection::Library),
    Page::Connections(ConnectionTab::GitHub, None),
  ] {
    let under = Route::Project(project, page).path();
    let over = import::open(&under, ImportTab::Tools);
    assert_eq!(Route::parse(&over), Some(Route::Project(project, page)), "{over}");
    assert_eq!(import::tab(&over), Some(ImportTab::Tools));
    assert_eq!(import::close(&over), under);
    assert_eq!(import::tab(&under), None);
    assert_eq!(
      import::open(&over, ImportTab::All),
      import::open(&under, ImportTab::All),
      "one import at a time"
    );
    let matched = routes(&Places::default()).resolve(&over);
    assert_ne!(matched.last().map(|leaf| leaf.pattern_raw()), Some("**"), "{over}");
  }
}

#[test]
fn route_all_projects_is_the_start_when_the_last_project_cannot_open() {
  let window = Window::new();
  window.arrive(Start::AllProjects);
  assert_eq!(window.path(), Route::AllProjects.path());
  window.router.push("/nowhere");
  assert_eq!(
    window.path(),
    Route::AllProjects.path(),
    "unknown places lead to All projects"
  );
}

#[test]
fn route_switching_projects_adds_entries_that_back_and_forward_revisit() {
  let (window, kontur) = Window::on_project();
  let website = window.project("personal-website");
  let website_id = website.project.id();
  window.navigation.show_all_projects();
  window.navigation.show_project(website);
  let paths = entries(&window.router);
  assert_eq!(
    paths,
    [
      Route::Project(kontur, Page::OVERVIEW).path(),
      Route::AllProjects.path(),
      Route::Project(website_id, Page::OVERVIEW).path(),
    ]
  );
}

#[test]
fn route_a_removed_project_leaves_history_for_all_projects() {
  let (window, kontur) = Window::on_project();
  let overview = Route::Project(kontur, Page::OVERVIEW).path();
  window.navigation.show_all_projects();
  window.navigation.forget_project(kontur);
  window.router.push(overview);
  assert_eq!(
    window.path(),
    Route::AllProjects.path(),
    "the removed project's screens lead to the start, All projects"
  );
}

#[test]
fn route_leaving_setup_returns_to_where_it_was_opened() {
  let (window, _) = Window::on_project();
  window.navigation.show_all_projects();
  window.navigation.open(Route::Setup, OpenIn::Current);
  window.navigation.leave_setup();
  assert_eq!(window.path(), Route::AllProjects.path());
}

#[test]
fn route_repository_import_opens_over_the_page_and_closes_to_it() {
  let project = ProjectId::generate();
  for page in [
    Page::Skills(None),
    Page::Instructions(InstructionsTab::AgentRoles, None),
    Page::Settings(SettingsTab::General),
  ] {
    let under = Route::Project(project, page).path();
    let over = repository_import::open(&under);
    assert_eq!(Route::parse(&over), Some(Route::Project(project, page)), "{over}");
    assert!(repository_import::shown(&over));
    assert!(!repository_import::shown(&under));
    assert_eq!(repository_import::close(&over), under);
    assert_eq!(repository_import::open(&over), over, "one import at a time");
  }
}
