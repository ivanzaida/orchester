//! The personal workspace: the application database and the projects it knows. Every method
//! blocks on SQLite or the file system; the UI calls them off the UI thread.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  appearance::ThemeSetting,
  id::{ProjectId, ServerId},
  project::Project,
  run::TaskPhase,
  time::Timestamp,
  usage::limits::WarningThreshold,
};
use orchester_store::{
  AppDb, PreviousSession, ProjectDb, RegisteredProject, ServerAccount, SessionEnd, SessionId, StoreError, StoredServer,
};
use parking_lot::Mutex;

use crate::{
  messages::{self, Message},
  project_setup::NewProject,
};

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

/// A project of the workspace as All projects lists it at once: from the application database
/// and whether its folder is still there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectEntry {
  pub project: RegisteredProject,
  pub folder_missing: bool,
}

/// What a listed project's own database says, read after the list is shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectData {
  /// Its project data cannot be read, and why; it cannot be opened.
  Unreadable(Message),
  /// It can be opened. The phase of each task, for its run counts; `None` when they cannot be
  /// read before the project is opened (its data waits for this version's migration) or reading
  /// them failed.
  Readable(Option<Arc<[TaskPhase]>>),
}

impl ProjectData {
  /// Reads the database of `project`, whose folder is there, without changing it.
  fn read(project: &RegisteredProject) -> Self {
    let db = match ProjectDb::open_read_only(&project.repo_root) {
      Ok(db) => db,
      // Opening the project migrates its data; until then its tasks cannot be read.
      Err(StoreError::SchemaTooOld { .. }) => return Self::Readable(None),
      Err(error) => {
        tracing::warn!(%error, repo_root = %project.repo_root.display(), "cannot read a listed project");
        return Self::Unreadable(messages::store_error(&error));
      }
    };
    match db.task_phases() {
      Ok(phases) => Self::Readable(Some(phases.into())),
      // The project still opens; only its counts are unknown, and the log says why.
      Err(error) => {
        tracing::error!(%error, repo_root = %project.repo_root.display(), "cannot read a listed project's runs");
        Self::Readable(None)
      }
    }
  }
}

/// The data of the listed projects whose folder is there, by project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListedData(HashMap<ProjectId, ProjectData>);

impl ListedData {
  pub fn read(entries: &[ProjectEntry]) -> Self {
    Self(
      entries
        .iter()
        .filter(|entry| !entry.folder_missing)
        .map(|entry| (entry.project.id, ProjectData::read(&entry.project)))
        .collect(),
    )
  }

  pub fn of(&self, project: ProjectId) -> Option<&ProjectData> {
    self.0.get(&project)
  }
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
    Ok(match self.open(&recent.repo_root) {
      Ok(project) => Landing::Project(project),
      Err(error) => Landing::Unavailable { project: recent, error },
    })
  }

  /// Known projects, most recently opened first.
  pub fn projects(&self) -> Result<Vec<RegisteredProject>, StoreError> {
    self.db.lock().projects()
  }

  /// Known projects, most recently opened first, with whether each one's folder is there.
  pub fn entries(&self) -> Result<Vec<ProjectEntry>, StoreError> {
    Ok(
      self
        .projects()?
        .into_iter()
        .map(|project| ProjectEntry {
          folder_missing: project.folder_missing(),
          project,
        })
        .collect(),
    )
  }

  /// Forgets `project`: only the application database changes, never its folder or its data.
  pub fn forget(&self, project: ProjectId) -> Result<(), StoreError> {
    self.db.lock().forget_project(project)
  }

  /// Opens the project Orchester keeps in the folder `repo_root` and records it as the most
  /// recently used one; a folder whose project was removed from the list is added again.
  pub fn open(&self, repo_root: &Path) -> Result<OpenProject, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let project = db.project().clone();
    self.db.lock().register_project(&project, repo_root, Timestamp::now())?;
    Ok(OpenProject {
      project,
      repo_root: repo_root.to_path_buf(),
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

  /// What an app saved under `key` of the window's state (`ui_state`), as it wrote it. Each app
  /// keeps its own keys, so two apps never read each other's tabs.
  pub fn ui_state(&self, key: &str) -> Result<Option<String>, StoreError> {
    self.db.lock().ui_state(key)
  }

  pub fn set_ui_state(&self, key: &str, text: &str) -> Result<(), StoreError> {
    self.db.lock().set_ui_state(key, text)
  }

  /// Whether automation may answer waiting tool requests through Orchester's MCP server (*Let
  /// automation answer approvals*); off until a person turns it on.
  pub fn automation_answers_approvals(&self) -> Result<bool, StoreError> {
    self.db.lock().automation_answers_approvals()
  }

  pub fn set_automation_answers_approvals(&self, allowed: bool) -> Result<(), StoreError> {
    self.db.lock().set_automation_answers_approvals(allowed)
  }

  /// Where Usage › Plan limits draws its warning line, for every project; 80 % until it is changed.
  pub fn usage_warning_threshold(&self) -> Result<WarningThreshold, StoreError> {
    self.db.lock().usage_warning_threshold()
  }

  pub fn set_usage_warning_threshold(&self, threshold: WarningThreshold) -> Result<(), StoreError> {
    self.db.lock().set_usage_warning_threshold(threshold)
  }

  /// The colors the window uses (`orchester-app`); [`ThemeSetting::default`] until one is chosen.
  pub fn theme(&self) -> Result<ThemeSetting, StoreError> {
    self.db.lock().theme()
  }

  pub fn set_theme(&self, theme: ThemeSetting) -> Result<(), StoreError> {
    self.db.lock().set_theme(theme)
  }

  /// Whether agents may control `orchester-app`'s window through its MCP server; `None` until it is
  /// set, when the build decides.
  pub fn mcp_window_control(&self) -> Result<Option<bool>, StoreError> {
    self.db.lock().mcp_window_control()
  }

  /// The shared servers, in the order they were added.
  pub fn servers(&self) -> Result<Vec<StoredServer>, StoreError> {
    self.db.lock().servers()
  }

  pub fn add_server(&self, server: &StoredServer) -> Result<(), StoreError> {
    self.db.lock().add_server(server)
  }

  /// Records who is signed in to `server` from this computer; `None` after signing out.
  pub fn set_server_account(&self, server: ServerId, account: Option<&ServerAccount>) -> Result<(), StoreError> {
    self.db.lock().set_server_account(server, account)
  }

  pub(crate) fn start_session(&self, now: Timestamp) -> Result<(SessionId, PreviousSession), StoreError> {
    self.db.lock().start_session(std::process::id(), now)
  }

  pub(crate) fn end_session(&self, session: SessionId, end: SessionEnd) -> Result<(), StoreError> {
    self.db.lock().end_session(session, end, Timestamp::now())
  }
}
