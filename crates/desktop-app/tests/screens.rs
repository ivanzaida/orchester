//! Every route's path shows its screen's own element, `screen-<name>`: an honest placeholder until the
//! screen is built. A path that names nothing shows `screen-not-found`, which says why. The paths go
//! straight to the router, as a tab's history would hand them over, so a path no route prints is shown
//! too. The project the paths name is known to be open; what the window does until a project opens,
//! and for one that cannot, is `tests/projects.rs`.

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
  mcp::McpStatus,
  navigation::{Navigation, tabs::TabSet},
  project::{ProjectState, Projects},
  routes::{
    ChangeSource, ChangedFile, OverviewTab, ProjectPage, Route, Screen, TaskList, TaskListTab, TaskTab, TaskView,
    routes,
  },
  theme::{self, Mode},
};
use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_workspace::workspace::OpenProject;
use parking_lot::Mutex;

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";
const NOT_BUILT: &str = "This screen isn't built yet.";

/// Where the test root leaves its router.
#[derive(Clone, Default)]
struct RouterSlot(Arc<Mutex<Option<RouterHandle>>>);

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
      projects: Projects::new(Some((project, ProjectState::Open(kontur(project))))),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    ctx.provide(McpStatus::Unavailable);
    ctx.provide(self.navigation.clone());
    ctx.provide(self.projects.clone());
    Router::mount(ctx, self.router.clone())
  }
}

struct Screens {
  app: App,
  tree: Tree,
  router: RouterHandle,
}

impl Screens {
  fn open() -> Self {
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    let slot = RouterSlot::default();
    tree.mount_root::<ScreensRoot>(&mut app, slot.clone());
    tree.pass_headless(&mut app);
    let router = slot.0.lock().take().expect("the root left its router");
    Self { app, tree, router }
  }

  /// The texts the screen at `path` shows under the element `id`, icon glyphs left out; `None` when
  /// no element has the id.
  fn show(&mut self, path: &str, id: &str) -> Option<Vec<String>> {
    self.router.replace(path);
    self.tree.pass_headless(&mut self.app);
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

/// A route of every screen, with the title its placeholder shows.
fn every_screen() -> Vec<(Route, &'static str)> {
  let changes = TaskView::Changes(Some(
    ChangedFile::new("src/a b.rs", ChangeSource::Worktree).expect("a file path"),
  ));
  let mut all = vec![(Route::AllProjects, "All projects"), (Route::Settings, "Settings")];
  all.extend([
    (at(ProjectPage::Overview(OverviewTab::Failed)), "Overview"),
    (
      at(ProjectPage::Tasks(
        TaskList::new(TaskListTab::Done)
          .with_search("retry & jitter")
          .with_highlight(key("KON-13")),
      )),
      "Tasks",
    ),
    (at(ProjectPage::NewTask), "New task"),
    (at(ProjectPage::EditTask(key("KON-7"))), "Edit KON-7"),
    (
      at(ProjectPage::Task(key("KON-12"), TaskView::of(TaskTab::Summary))),
      "KON-12 · Summary",
    ),
    (at(ProjectPage::Task(key("KON-12"), changes)), "KON-12 · Changes"),
    (at(ProjectPage::PullRequests), "Pull requests"),
    (at(ProjectPage::Usage), "Usage"),
    (at(ProjectPage::Pipelines), "Pipelines"),
    (at(ProjectPage::Agents), "Agents"),
    (at(ProjectPage::Instructions), "Instructions"),
    (at(ProjectPage::Skills), "Skills"),
    (at(ProjectPage::Tools), "Tools"),
    (at(ProjectPage::Secrets), "Secrets"),
    (at(ProjectPage::Connections), "Connections"),
    (at(ProjectPage::Settings), "Project settings"),
  ]);
  all
}

#[test]
fn screens_each_route_mounts_its_own_placeholder() {
  let mut screens = Screens::open();
  let routes = every_screen();
  for screen in Screen::ALL {
    assert!(
      routes.iter().any(|(route, _)| route.screen() == screen),
      "{screen:?} is covered"
    );
  }
  for (route, title) in routes {
    let id = format!("screen-{}", route.screen().name());
    let texts = screens
      .show(&route.path(), &id)
      .unwrap_or_else(|| panic!("{} shows {id}", route.path()));
    assert_eq!(texts, [title, NOT_BUILT], "{}", route.path());
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
