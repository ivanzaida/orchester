//! Which screen a tab shows: the app's routes on lurq's router, with back and forward history.
//! Each tab's host owns a router and provides [`Navigation`] to the screens in it; the project
//! shell adds its own navigation for the pages inside it.

pub(crate) mod entry;
mod history;
pub mod import;
pub mod route;
mod table;

use std::{collections::HashMap, sync::Arc};

use lurq::router::RouterHandle;
use orchester_core::id::ProjectId;
use parking_lot::Mutex;

use self::route::{Page, Route};
pub use self::{
  history::forward,
  table::{PROJECT_PARAM, routes},
};
use crate::{
  tabs::{OpenIn, Tabs},
  workspace::{Landing, OpenProject, Workspace},
};

/// The screen the application starts on.
#[derive(Clone, Debug, PartialEq)]
pub enum Start {
  Welcome,
  Project(OpenProject),
  /// The most recently used project could not be opened; All projects says why.
  AllProjects,
}

impl lurq::app::component::DevtoolsInspectable for Start {}

impl From<Landing> for Start {
  fn from(landing: Landing) -> Self {
    match landing {
      Landing::FirstLaunch => Self::Welcome,
      Landing::Project(project) => Self::Project(project),
      Landing::Unavailable { project, error } => {
        tracing::error!(%error, repo_root = %project.repo_root.display(), "cannot open the most recent project");
        Self::AllProjects
      }
    }
  }
}

/// What routes stand for beyond their path: the projects opened in this session, by id, and
/// the start screen once it is known. Shared by the root, the route guards and the screens.
#[derive(Clone, Default)]
pub struct Places(Arc<Mutex<Known>>);

#[derive(Default)]
struct Known {
  projects: HashMap<ProjectId, OpenProject>,
  start: Option<Route>,
}

impl PartialEq for Places {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Places {
  pub fn project(&self, id: ProjectId) -> Option<OpenProject> {
    self.0.lock().projects.get(&id).cloned()
  }

  pub(crate) fn add_project(&self, project: OpenProject) {
    self.0.lock().projects.insert(project.project.id(), project);
  }

  /// The route of the start screen; `None` until it is known.
  pub fn start(&self) -> Option<Route> {
    self.0.lock().start
  }

  /// Records `start` as the start screen and returns its route.
  pub fn set_start(&self, start: Start) -> Route {
    let route = match start {
      Start::Welcome => Route::Welcome,
      Start::Project(project) => {
        let route = Route::Project(project.project.id(), Page::OVERVIEW);
        self.add_project(project);
        route
      }
      Start::AllProjects => Route::AllProjects,
    };
    self.0.lock().start = Some(route);
    route
  }

  /// A project was created: it is where the application now starts, and the first-launch screen
  /// no longer applies.
  fn created(&self, project: OpenProject) -> Route {
    let route = Route::Project(project.project.id(), Page::OVERVIEW);
    let mut known = self.0.lock();
    known.projects.insert(project.project.id(), project);
    known.start = Some(route);
    route
  }

  /// A project was removed from the workspace: its screens no longer exist, and when the
  /// application started on it, All projects takes its place as the start.
  fn forget(&self, project: ProjectId) {
    let mut known = self.0.lock();
    known.projects.remove(&project);
    if matches!(known.start, Some(Route::Project(started, _)) if started == project) {
      known.start = Some(Route::AllProjects);
    }
  }

  /// Whether `route`, a screen outside any project, still applies: the welcome screen only
  /// while no project was opened.
  pub fn applies(&self, route: Route) -> bool {
    match route {
      Route::Setup | Route::AllProjects => true,
      Route::Welcome => self.start() == Some(Route::Welcome),
      Route::Start | Route::Project(..) => false,
    }
  }
}

/// Provided by each tab's host: the workspace, and going between the screens of that tab and to
/// new tabs.
#[derive(Clone)]
pub struct Navigation {
  workspace: Workspace,
  router: RouterHandle,
  places: Places,
  tabs: Tabs,
}

impl Navigation {
  pub fn new(workspace: Workspace, router: RouterHandle, places: Places, tabs: Tabs) -> Self {
    Self {
      workspace,
      router,
      places,
      tabs,
    }
  }

  pub fn workspace(&self) -> &Workspace {
    &self.workspace
  }

  pub fn router(&self) -> &RouterHandle {
    &self.router
  }

  pub fn places(&self) -> &Places {
    &self.places
  }

  /// The window's tabs.
  pub fn tabs(&self) -> &Tabs {
    &self.tabs
  }

  pub fn show_setup(&self) {
    entry::push(&self.router.navigator(), Route::Setup.path());
  }

  /// Back to the screen chosen at startup.
  pub fn back_to_start(&self) {
    entry::push(&self.router.navigator(), Route::Start.path());
  }

  /// Leaves setup without creating a project: back to the screen it was opened from (All
  /// projects, a project's switcher, the welcome screen), or the start screen when that is not
  /// known.
  pub fn leave_setup(&self) {
    let navigator = self.router.navigator();
    if entry::from(&navigator).is_some() {
      navigator.back();
    } else {
      self.back_to_start();
    }
  }

  pub fn show_all_projects(&self) {
    entry::push(&self.router.navigator(), Route::AllProjects.path());
  }

  /// Shows `project`, just opened from All projects or the switcher, as a new history entry of
  /// this tab. The project shown before keeps its runner while another tab shows it or it has
  /// work (see `Runners::keep_open`).
  pub fn show_project(&self, project: OpenProject) {
    self.open_project(project, OpenIn::Current);
  }

  /// Opens `project`'s overview where `open` says: this tab, or a new tab.
  pub fn open_project(&self, project: OpenProject, open: OpenIn) {
    let route = Route::Project(project.project.id(), Page::OVERVIEW);
    self.places.add_project(project);
    match open {
      OpenIn::Current => entry::push(&self.router.navigator(), route.path()),
      OpenIn::Background | OpenIn::NewTab => self.tabs.open(route, open),
    }
  }

  /// `project` was removed from the workspace: history entries of its screens lead to the start.
  pub fn forget_project(&self, project: ProjectId) {
    self.places.forget(project);
  }

  /// Opens a project that was just created: its overview takes the place of the setup form,
  /// so going back does not return to a form that was already submitted. The welcome screen
  /// setup was opened from no longer applies and shows the project too, so history returns to
  /// that entry instead of holding the project twice.
  pub fn open_created(&self, project: OpenProject) {
    let route = self.places.created(project);
    let navigator = self.router.navigator();
    if entry::from(&navigator) == Some(Route::Welcome) {
      entry::finish_back(&navigator);
    }
    entry::replace(&navigator, route.path());
  }
}
