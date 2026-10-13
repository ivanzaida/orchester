//! What the tools work with: the same workspace, runners, servers and programs the screens use, so a
//! tool goes through the paths a person's action goes through, with the same rules and records.

use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::error::{Missing, ToolError};
use crate::{
  activity::Activity,
  messages::Message,
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::{OpenProject, Workspace},
};

#[derive(Clone)]
pub struct Services {
  pub(super) workspace: Workspace,
  pub(super) runners: Runners,
  pub(super) servers: Servers,
  pub(super) programs: Programs,
  pub(super) vault: Vault,
  /// Told what each tool changed in which project, so only the screens showing it read it again.
  pub(super) changes: OutsideChanges,
  /// The status bar's operations, which list a tool's git work as they list a person's.
  pub(super) activity: Activity,
  /// The projects' merge and removal operations, shared with the task pages.
  pub(super) jobs: ProjectJobs,
}

impl Services {
  pub fn new(
    workspace: Workspace,
    runners: Runners,
    servers: Servers,
    programs: Programs,
    (changes, activity, jobs): (OutsideChanges, Activity, ProjectJobs),
  ) -> Self {
    Self {
      workspace,
      runners,
      servers,
      programs,
      vault: Vault::keychain(),
      changes,
      activity,
      jobs,
    }
  }

  pub fn with_vault(mut self, vault: Vault) -> Self {
    self.vault = vault;
    self
  }

  /// The project `named` by its id or, ignoring case, its name, opened as the app opens it. Blocks on
  /// SQLite. It is not recorded as the most recently used project: only a person opening it is.
  pub(super) fn project(&self, named: &str) -> Result<OpenProject, ToolError> {
    let projects = self.workspace.projects()?;
    let by_id = projects.iter().find(|project| project.id.to_string() == named);
    let lowered = named.to_lowercase();
    let by_name: Vec<_> = projects
      .iter()
      .filter(|project| project.name.as_str().to_lowercase() == lowered)
      .collect();
    let registered = match (by_id, by_name.as_slice()) {
      (Some(project), _) => project,
      (None, [project]) => *project,
      (None, []) => return Err(ToolError::not_found(Missing::Project, named)),
      (None, _) => return Err(ToolError::AmbiguousProject(named.to_owned())),
    };
    if registered.folder_missing() {
      return Err(ToolError::refused(
        "project_folder_missing",
        vec![Message::new("errors.projectMissing")],
      ));
    }
    let db = ProjectDb::open(&registered.repo_root)?;
    Ok(OpenProject {
      project: db.project().clone(),
      repo_root: registered.repo_root.clone(),
    })
  }
}

/// Runs `work`, which blocks on SQLite or git, on the blocking pool.
pub(super) async fn blocking<T: Send + 'static>(
  work: impl FnOnce() -> Result<T, ToolError> + Send + 'static,
) -> Result<T, ToolError> {
  tokio::task::spawn_blocking(work).await.map_err(|error| {
    tracing::error!(%error, "an MCP tool's work did not finish");
    ToolError::interrupted()
  })?
}
