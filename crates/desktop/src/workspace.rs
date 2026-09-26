//! The personal workspace: the application database and the projects it knows. Every method
//! blocks on SQLite or the file system; the UI calls them off the UI thread.

use std::{path::PathBuf, sync::Arc};

use orchester_core::{project::Project, time::Timestamp};
use orchester_store::{AppDb, PreviousSession, ProjectDb, RegisteredProject, SessionEnd, SessionId, StoreError};
use parking_lot::Mutex;

use crate::project_setup::NewProject;

/// A project whose database is open and readable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenProject {
  pub project: Project,
  pub repo_root: PathBuf,
}

/// What to show when the application starts.
pub enum Landing {
  /// No project is known yet.
  FirstLaunch,
  /// The most recently used project.
  Project(OpenProject),
  /// The most recently used project exists in the workspace but cannot be opened.
  Unavailable {
    project: RegisteredProject,
    error: StoreError,
  },
}

/// Shared handle to the application database.
#[derive(Clone)]
pub struct Workspace {
  db: Arc<Mutex<AppDb>>,
}

impl PartialEq for Workspace {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.db, &other.db)
  }
}

impl lurq::app::component::DevtoolsInspectable for Workspace {}

impl Workspace {
  pub fn new(db: AppDb) -> Self {
    Self {
      db: Arc::new(Mutex::new(db)),
    }
  }

  pub fn landing(&self) -> Result<Landing, StoreError> {
    let Some(recent) = self.db.lock().projects()?.into_iter().next() else {
      return Ok(Landing::FirstLaunch);
    };
    Ok(match self.open(&recent) {
      Ok(project) => Landing::Project(project),
      Err(error) => Landing::Unavailable { project: recent, error },
    })
  }

  /// Known projects, most recently opened first.
  pub fn projects(&self) -> Result<Vec<RegisteredProject>, StoreError> {
    self.db.lock().projects()
  }

  /// Opens a known project and records it as the most recently used one.
  pub fn open(&self, registered: &RegisteredProject) -> Result<OpenProject, StoreError> {
    let db = ProjectDb::open(&registered.repo_root)?;
    let project = db.project().clone();
    self
      .db
      .lock()
      .register_project(&project, &registered.repo_root, Timestamp::now())?;
    Ok(OpenProject {
      project,
      repo_root: registered.repo_root.clone(),
    })
  }

  /// Initializes Orchester in the repository and adds the project to the workspace.
  pub fn create(&self, new: NewProject) -> Result<OpenProject, StoreError> {
    ProjectDb::create(&new.repo_root, new.project.clone())?;
    self
      .db
      .lock()
      .register_project(&new.project, &new.repo_root, Timestamp::now())?;
    Ok(OpenProject {
      project: new.project,
      repo_root: new.repo_root,
    })
  }

  pub(crate) fn start_session(&self, now: Timestamp) -> Result<(SessionId, PreviousSession), StoreError> {
    self.db.lock().start_session(std::process::id(), now)
  }

  pub(crate) fn end_session(&self, session: SessionId, end: SessionEnd) -> Result<(), StoreError> {
    self.db.lock().end_session(session, end, Timestamp::now())
  }
}

impl lurq::app::component::DevtoolsInspectable for OpenProject {}
