//! Which screen the window shows: the app's routes on lurq's router, with back and forward
//! history. The root owns the router and provides [`Navigation`] to every screen; the project
//! shell adds its own navigation for the pages inside it.

pub(crate) mod entry;
mod history;
pub mod route;
mod table;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use lurq::router::RouterHandle;
use orchester_core::id::ProjectId;
use parking_lot::Mutex;

use self::route::{Page, Route};
pub use self::{
  history::{forward, with_history_input},
  table::{PROJECT_PARAM, routes},
};
use crate::{
  messages::{self, Message},
  workspace::{Landing, OpenProject, Workspace},
};

/// The most recently used project, which could not be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnavailableProject {
  pub name: Arc<str>,
  pub repo_root: PathBuf,
  pub reason: Message,
}

impl lurq::app::component::DevtoolsInspectable for UnavailableProject {}

/// The screen the application starts on.
#[derive(Clone, Debug, PartialEq)]
pub enum Start {
  Welcome,
  Project(OpenProject),
  Unavailable(UnavailableProject),
}

impl lurq::app::component::DevtoolsInspectable for Start {}

impl From<Landing> for Start {
  fn from(landing: Landing) -> Self {
    match landing {
      Landing::FirstLaunch => Self::Welcome,
      Landing::Project(project) => Self::Project(project),
      Landing::Unavailable { project, error } => {
        tracing::error!(%error, repo_root = %project.repo_root.display(), "cannot open the most recent project");
        Self::Unavailable(UnavailableProject {
          name: project.name.as_str().into(),
          repo_root: project.repo_root,
          reason: messages::store_error(&error),
        })
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
  unavailable: Option<UnavailableProject>,
}

impl Places {
  pub fn project(&self, id: ProjectId) -> Option<OpenProject> {
    self.0.lock().projects.get(&id).cloned()
  }

  fn add_project(&self, project: OpenProject) {
    self.0.lock().projects.insert(project.project.id(), project);
  }

  /// The route of the start screen; `None` until it is known.
  pub fn start(&self) -> Option<Route> {
    self.0.lock().start
  }

  /// The project the start screen reports as unavailable, if it does.
  pub fn unavailable(&self) -> Option<UnavailableProject> {
    self.0.lock().unavailable.clone()
  }

  /// Records `start` and returns its route.
  fn set_start(&self, start: Start) -> Route {
    let route = match start {
      Start::Welcome => Route::Welcome,
      Start::Project(project) => {
        let route = Route::Project(project.project.id(), Page::OVERVIEW);
        self.add_project(project);
        route
      }
      Start::Unavailable(project) => {
        self.0.lock().unavailable = Some(project);
        Route::Unavailable
      }
    };
    self.0.lock().start = Some(route);
    route
  }

  /// A project was created: it is where the application now starts, and the first-launch and
  /// unavailable screens no longer apply.
  fn created(&self, project: OpenProject) -> Route {
    let route = Route::Project(project.project.id(), Page::OVERVIEW);
    let mut known = self.0.lock();
    known.projects.insert(project.project.id(), project);
    known.start = Some(route);
    known.unavailable = None;
    route
  }

  /// Whether `route`, a screen outside any project, still applies: the welcome screen only
  /// while no project was opened, the unavailable project only while the start reports it.
  pub fn applies(&self, route: Route) -> bool {
    match route {
      Route::Setup => true,
      Route::Welcome => self.start() == Some(Route::Welcome),
      Route::Unavailable => self.unavailable().is_some(),
      Route::Start | Route::Project(..) => false,
    }
  }
}

/// Provided by the root: the workspace, and going between the screens outside a project.
#[derive(Clone)]
pub struct Navigation {
  workspace: Workspace,
  router: RouterHandle,
  places: Places,
}

impl Navigation {
  pub fn new(workspace: Workspace, router: RouterHandle, places: Places) -> Self {
    Self {
      workspace,
      router,
      places,
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

  /// The start screen became known: the window leaves the empty start route for it, in place,
  /// so going back never returns to the empty window. Later calls change nothing.
  pub fn arrive(&self, start: Start) {
    if self.places.start().is_some() {
      return;
    }
    let route = self.places.set_start(start);
    let path = self.router.path().get_untracked();
    if path.is_empty() || Route::parse(&path) == Some(Route::Start) {
      self.router.replace(route.path());
    }
  }

  pub fn show_setup(&self) {
    entry::push(&self.router.navigator(), Route::Setup.path());
  }

  /// Back to the screen chosen at startup.
  pub fn back_to_start(&self) {
    entry::push(&self.router.navigator(), Route::Start.path());
  }

  /// Opens a project that was just created: its overview takes the place of the setup form,
  /// so going back does not return to a form that was already submitted. The welcome or
  /// unavailable screen setup was opened from no longer applies and shows the project too, so
  /// history returns to that entry instead of holding the project twice.
  pub fn open_project(&self, project: OpenProject) {
    let route = self.places.created(project);
    let navigator = self.router.navigator();
    if matches!(entry::from(&navigator), Some(Route::Welcome | Route::Unavailable)) {
      entry::finish_back(&navigator);
    }
    entry::replace(&navigator, route.path());
  }
}
