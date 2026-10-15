//! The personal workspace: the application database and the projects it knows. Every method
//! blocks on SQLite or the file system; the UI calls them off the UI thread.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::{Arc, atomic::AtomicBool},
};

use orchester_core::{
  app_mcp::McpPort,
  appearance::ThemeSetting,
  id::{InstallationId, ProjectId, ServerId},
  project::Project,
  time::Timestamp,
  usage::limits::WarningThreshold,
};
use orchester_exec::programs::Programs;
use orchester_store::{
  AppDb, PreviousSession, ProjectDb, RegisteredProject, ServerAccount, SessionEnd, SessionId, StoreError, StoredServer,
};
use parking_lot::Mutex;

use crate::{
  messages::{self, Message},
  project_setup::NewProject,
  tasks::data::ProjectTasks,
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
  /// It can be opened. Its tasks, for its run counts (`project_list::RunCounts`); `None` when they cannot be
  /// read before the project is opened (its data waits for this version's migration) or reading them failed.
  Readable(Option<Arc<ProjectTasks>>),
}

impl ProjectData {
  /// Reads the database of `project`, whose folder is there, without changing it; git, through `programs`,
  /// tells the runs merged outside Orchester.
  fn read(project: &RegisteredProject, programs: &Programs) -> Self {
    let db = match ProjectDb::open_read_only(&project.repo_root) {
      Ok(db) => db,
      // Opening the project migrates its data; until then its tasks cannot be read.
      Err(StoreError::SchemaTooOld { .. }) => return Self::Readable(None),
      Err(error) => {
        tracing::warn!(%error, repo_root = %project.repo_root.display(), "cannot read a listed project");
        return Self::Unreadable(messages::store_error(&error));
      }
    };
    match ProjectTasks::read(&db, &project.repo_root, programs) {
      Ok(tasks) => Self::Readable(Some(Arc::new(tasks))),
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
  pub fn read(entries: &[ProjectEntry], programs: &Programs) -> Self {
    Self(
      entries
        .iter()
        .filter(|entry| !entry.folder_missing)
        .map(|entry| (entry.project.id, ProjectData::read(&entry.project, programs)))
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
  pub(crate) db: Arc<Mutex<AppDb>>,
  /// Where Orchester creates new agent accounts' folders; `None` where it has none on this platform.
  pub(crate) account_root: Option<Arc<Path>>,
  /// Where removed accounts' folders go when the person asks; none until the app gives one.
  pub(crate) trash: crate::accounts::trash::SharedTrash,
  permission_consent: Arc<Mutex<Consent>>,
  approval_consent: Arc<Mutex<Consent>>,
  /// Whether All projects listed a project in this session (`project_list`).
  pub(crate) listed: Arc<AtomicBool>,
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
      account_root: account_root().map(Arc::from),
      trash: crate::accounts::trash::SharedTrash::default(),
      permission_consent: Arc::default(),
      approval_consent: Arc::default(),
      listed: Arc::default(),
    }
  }

  /// Gives this workspace and its clones `trash` for removed accounts' folders: the system's in the app's window
  /// only, a folder of its own in a QA copy or a test.
  pub fn use_trash(&self, trash: Arc<dyn crate::accounts::trash::Trash>) {
    self.trash.set(trash);
  }

  /// This workspace with new agent accounts' folders created in `root` instead of Orchester's application data.
  #[must_use]
  pub fn with_account_root(mut self, root: &Path) -> Self {
    self.account_root = Some(Arc::from(root));
    self
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
    // The project opens all the same; its connections follow the removal the next time it opens.
    if let Err(error) = self.follow_accounts(repo_root) {
      tracing::warn!(%error, "cannot move a project's connections off removed accounts");
    }
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

  pub fn remove_ui_state(&self, key: &str) -> Result<(), StoreError> {
    self.db.lock().remove_ui_state(key)
  }

  /// The keys of the window's state that start with `prefix`.
  pub fn ui_state_keys(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
    self.db.lock().ui_state_keys(prefix)
  }

  /// Whether automation may answer waiting tool requests through Orchester's MCP server (*Let
  /// automation answer approvals*); off until a person turns it on.
  pub fn automation_answers_approvals(&self) -> Result<bool, StoreError> {
    self.approval_consent.lock().begin();
    let db = self.db.lock();
    let result = db.automation_answers_approvals();
    self.approval_consent.lock().finish(matches!(&result, Ok(true)));
    result
  }

  pub fn set_automation_answers_approvals(&self, allowed: bool) -> Result<(), StoreError> {
    {
      let mut consent = self.approval_consent.lock();
      if !allowed {
        consent.revision += 1;
      }
      consent.begin();
    }
    let mut db = self.db.lock();
    let result = db.set_automation_answers_approvals(allowed);
    self.approval_consent.lock().finish(allowed && result.is_ok());
    result
  }

  /// Whether automation may widen what agents may do through Orchester's MCP server (*Let automation
  /// change agent permissions*); off until a person turns it on in the window, and no tool changes it.
  pub fn automation_changes_permissions(&self) -> Result<bool, StoreError> {
    self.permission_consent.lock().begin();
    let db = self.db.lock();
    let result = db.automation_changes_permissions();
    let mut consent = self.permission_consent.lock();
    consent.finish(matches!(&result, Ok(true)));
    result.map(|_| consent.allowed)
  }

  pub fn set_automation_changes_permissions(&self, allowed: bool) -> Result<(), StoreError> {
    let revision = {
      let mut consent = self.permission_consent.lock();
      if !allowed {
        consent.revoke();
      }
      consent.begin();
      consent.revision
    };
    let mut db = self.db.lock();
    let result = db.set_automation_changes_permissions(allowed);
    let mut consent = self.permission_consent.lock();
    if result.is_ok() && consent.revision == revision {
      consent.revoked = !allowed;
    }
    consent.finish(allowed && result.is_ok());
    result
  }

  /// The last verified consent, without SQLite work on the UI thread; unknown/error/pending is false.
  pub fn automation_permission_consent(&self) -> bool {
    self.permission_consent.lock().allowed
  }

  /// The approvals opt-in remains separate from permission changes.
  pub fn automation_approval_consent(&self) -> bool {
    self.approval_consent.lock().allowed
  }

  /// Queued input can bind to revocation without treating these revisions as a grant.
  pub fn automation_consent_revision(&self) -> (u64, u64) {
    (
      self.permission_consent.lock().revision,
      self.approval_consent.lock().revision,
    )
  }

  /// A human revocation stops window widening before its asynchronous save is polled.
  pub fn revoke_automation_permission_consent(&self) {
    self.permission_consent.lock().revoke();
  }

  /// A failed or uncompleted authority read never retains an earlier grant.
  pub fn invalidate_automation_consent(&self) {
    self.permission_consent.lock().allowed = false;
    self.approval_consent.lock().allowed = false;
  }

  /// Whether the Orchestrator chat publishes the drafts it saved itself (*Let the Orchestrator publish drafts*);
  /// off until a person turns it on in the window, and no tool changes it.
  pub fn orchestrator_publishes_drafts(&self) -> Result<bool, StoreError> {
    self.db.lock().orchestrator_publishes_drafts()
  }

  pub fn set_orchestrator_publishes_drafts(&self, allowed: bool) -> Result<(), StoreError> {
    self.db.lock().set_orchestrator_publishes_drafts(allowed)
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

  /// Whether the installed app's MCP server listens; on until a person turns it off in Settings.
  pub fn mcp_server_enabled(&self) -> Result<bool, StoreError> {
    self.db.lock().mcp_server_enabled()
  }

  pub fn set_mcp_server_enabled(&self, enabled: bool) -> Result<(), StoreError> {
    self.db.lock().set_mcp_server_enabled(enabled)
  }

  /// The port the installed app's MCP server listens on.
  pub fn mcp_port(&self) -> Result<McpPort, StoreError> {
    self.db.lock().mcp_port()
  }

  pub fn set_mcp_port(&self, port: McpPort) -> Result<(), StoreError> {
    self.db.lock().set_mcp_port(port)
  }

  /// This installation's id, which names its secrets of its own in the keychain (the app's MCP token).
  pub fn installation_id(&self) -> Result<InstallationId, StoreError> {
    self.db.lock().installation_id()
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

  pub(crate) fn session_started_before(&self, session: SessionId) -> Result<Option<Timestamp>, StoreError> {
    self.db.lock().session_started_before(session)
  }

  pub(crate) fn end_session(&self, session: SessionId, end: SessionEnd) -> Result<(), StoreError> {
    self.db.lock().end_session(session, end, Timestamp::now())
  }
}

/// Published under the database lock; any pending read/write keeps an earlier value from authorizing the window.
#[derive(Default)]
struct Consent {
  allowed: bool,
  /// A human revocation survives canceled/failed dispatch and every old stored read.
  revoked: bool,
  revision: u64,
  pending: usize,
}
impl Consent {
  fn refresh(&mut self, allowed: bool) {
    self.allowed = self.pending == 0 && !self.revoked && allowed;
  }
  fn revoke(&mut self) {
    self.revision += 1;
    self.revoked = true;
    self.allowed = false;
  }
  fn begin(&mut self) {
    self.pending += 1;
    self.allowed = false;
  }
  fn finish(&mut self, allowed: bool) {
    self.pending -= 1;
    self.refresh(allowed);
  }
}

/// Where new agent accounts' folders go: `ORCHESTER_ACCOUNTS_DIR` when a QA copy or a test names one, so it never
/// writes the person's own account folders, else Orchester's application data (`location::accounts_dir`). The
/// installed app ignores the variable ([`crate::build::Build`]); one that is not a plain full path gives no folder.
fn account_root() -> Option<std::path::PathBuf> {
  let named = std::env::var_os("ORCHESTER_ACCOUNTS_DIR").filter(|folder| !folder.is_empty());
  if let Some(folder) = named.filter(|_| crate::build::Build::this() == crate::build::Build::Other) {
    let folder = std::path::PathBuf::from(folder);
    if orchester_core::account::plain_full_path(&folder) {
      return Some(folder);
    }
    tracing::error!(folder = %folder.display(), "ORCHESTER_ACCOUNTS_DIR is not a plain full path; no folder for new agent accounts");
    return None;
  }
  match orchester_store::location::accounts_dir() {
    Ok(folder) => Some(folder),
    Err(error) => {
      tracing::warn!(%error, "no folder for new agent accounts");
      None
    }
  }
}
