//! The projects the window shows. A route names a project by its id (`routes::Route::Project`); what
//! that id stands for in the workspace is its [`ProjectState`]: open, or unavailable because it is not
//! in the list, its folder is gone or its data cannot be read ([`resolve`]). The session opens where the
//! person left off ([`landing`]).
//!
//! The states of the projects the tabs show are the window's ([`Projects`]): [`ProjectsHost`] opens each
//! one once, keeps the runners of the open ones running, and keeps one [`ProjectFeed`] per open project
//! current. Screens get an open project as a [`ShownProject`].

mod daily_report;
pub mod feed;
mod feedback;
pub mod hold;
mod host;
mod known;
pub(crate) mod merge_queue;
pub mod own;
mod state;
mod waits;

use orchester_core::id::ProjectId;
use orchester_store::StoreError;
use orchester_workspace::workspace::{self, OpenProject, Workspace};

pub use self::{
  feed::{ProjectFeed, TasksState},
  feedback::feedback_holds,
  host::ProjectsHost,
  known::Projects,
  state::{ProjectState, Unavailable},
};
use crate::routes::{OverviewTab, ProjectPage, Route};

/// What `project` stands for in `workspace`. An open project is opened as a person opening it does:
/// its data is migrated if needed, and it becomes the most recently used project. Blocks on SQLite and
/// the file system.
pub fn resolve(workspace: &Workspace, project: ProjectId) -> Result<ProjectState, StoreError> {
  let Some(registered) = workspace
    .projects()?
    .into_iter()
    .find(|registered| registered.id == project)
  else {
    return Ok(ProjectState::Unavailable(Unavailable::Unknown));
  };
  if registered.folder_missing() {
    return Ok(ProjectState::Unavailable(Unavailable::FolderMissing {
      name: registered.name,
      repo_root: registered.repo_root,
    }));
  }
  Ok(match workspace.open(&registered.repo_root) {
    Ok(open) if open.project.id() == project => ProjectState::Open(open),
    // The folder holds another project now (it was set up again): the id names nothing there.
    Ok(other) => {
      tracing::warn!(found = %other.project.id(), "a project's folder holds another project");
      ProjectState::Unavailable(Unavailable::Unknown)
    }
    Err(error) => ProjectState::Unavailable(Unavailable::of(registered, &error)),
  })
}

/// Where a session opens, and what the project it opens on stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landing {
  pub route: Route,
  /// The project of `route`, as opening it found it; `None` outside a project.
  pub project: Option<(ProjectId, ProjectState)>,
}

impl lurq::app::component::DevtoolsInspectable for Landing {}

impl Landing {
  /// All projects, where a first launch opens.
  pub const fn all_projects() -> Self {
    Self {
      route: Route::AllProjects,
      project: None,
    }
  }
}

/// The most recently used project's Overview, also when it cannot be opened (its screen says why), or
/// All projects while the workspace has no project. Blocks on SQLite.
pub fn landing(workspace: &Workspace) -> Result<Landing, StoreError> {
  let (id, state) = match workspace.landing()? {
    workspace::Landing::FirstLaunch => return Ok(Landing::all_projects()),
    workspace::Landing::Project(open) => (open.project.id(), ProjectState::Open(open)),
    workspace::Landing::Unavailable { project, error } => {
      (project.id, ProjectState::Unavailable(Unavailable::of(project, &error)))
    }
  };
  Ok(Landing {
    route: Route::Project(id, ProjectPage::Overview(OverviewTab::default())),
    project: Some((id, state)),
  })
}

/// An open project as its screens get it: where it is, and what the window knows of it.
#[derive(Clone, PartialEq)]
pub struct ShownProject {
  pub open: OpenProject,
  pub feed: ProjectFeed,
}

impl lurq::app::component::DevtoolsInspectable for ShownProject {}

impl ShownProject {
  /// Its tasks as they stand with its runner ([`ProjectFeed::tasks_with_runner`]).
  pub fn tasks(&self) -> TasksState {
    self.feed.tasks_with_runner(&self.open.repo_root)
  }
}
