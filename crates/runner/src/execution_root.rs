//! The executor's own folders, all derived here from its execution root (`docs/architecture/remote-runners.md`,
//! rule 6): the run worktrees and step worktrees, the warm workspaces, the runs' own storage, the step executions'
//! skills, the tools given to agents, the codebase indexes and the copies of the files attached to messages that its
//! agents are given. Locally the execution root is the project's repository, so each folder is where it always was,
//! under its `.orchester/`.
//!
//! The executor's folders outside the repository belong to the computer it runs on, the same for every execution
//! root there: the private files with secret values and the user's data folder that holds the apps' discovery
//! folders and the QA slots.
//!
//! The folders of the person's checkout — `runner.lock`, the logs, the project database, attachments and blobs — are
//! the coordinator's and stay joined to the project root. The executor gets an attachment's content through the
//! coordinator link and keeps its own copy ([`ExecutionRoot::attachments`]): locally that is the project's own
//! attachments folder, where the copy already is.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::{RunId, StepExecutionId},
  integration::WORKTREES_DIR,
  workspaces::{WorkspaceNumber, artifacts_folder},
};
use orchester_index::CodebaseIndexes;

/// The executor's state folder in its execution root.
const STATE_DIR: &str = ".orchester";
/// The app's discovery folder in a user data folder, the computer's or a QA profile's.
pub const APP_DISCOVERY: &str = "orchester-app-mcp";

/// Where an executor keeps its folders: locally, the project's repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionRoot {
  repository: PathBuf,
}

impl ExecutionRoot {
  /// The execution root of a local executor: the project's repository at `repository`.
  pub fn local(repository: &Path) -> Self {
    Self {
      repository: repository.to_path_buf(),
    }
  }

  /// The repository the executor makes its worktrees and workspaces in.
  pub fn repository(&self) -> &Path {
    &self.repository
  }

  /// The worktree folder `name`: a run's, named after the run, or a step execution's, named after the execution.
  pub fn worktree(&self, name: &str) -> PathBuf {
    self.repository.join(WORKTREES_DIR).join(name)
  }

  /// The run `run`'s own worktree, from before workspaces.
  pub fn run_worktree(&self, run: RunId) -> PathBuf {
    self.worktree(&run.to_string())
  }

  /// The folder of warm workspace `number`.
  pub fn workspace(&self, number: WorkspaceNumber) -> PathBuf {
    number.path(&self.repository)
  }

  /// The run's own storage, `.orchester/runs/<run>/` (ORC-114).
  pub fn run_storage(&self, run: RunId) -> PathBuf {
    artifacts_folder(&self.repository, run)
  }

  /// The skills folder of step execution `execution`, `.orchester/skills/<execution>/`.
  pub fn skills(&self, execution: StepExecutionId) -> PathBuf {
    self
      .repository
      .join(STATE_DIR)
      .join("skills")
      .join(execution.to_string())
  }

  /// The folder of the programs given to agents, `.orchester/tools/`.
  pub fn tools(&self) -> PathBuf {
    self.repository.join(STATE_DIR).join("tools")
  }

  /// The codebase indexes, in `.orchester/index/`.
  pub fn code_indexes(&self) -> CodebaseIndexes {
    CodebaseIndexes::new(&self.repository)
  }

  /// The copies of the files attached to messages that the executor gives its agents, `.orchester/attachments/`:
  /// locally the project's own copies.
  pub fn attachments(&self) -> PathBuf {
    self.repository.join(STATE_DIR).join("attachments")
  }
}

/// `.orchester/worktrees/<name>` as git reads it in the repository's folder: relative, so a repository path that is
/// not valid text still works.
pub fn relative_worktree(name: &str) -> String {
  format!("{WORKTREES_DIR}/{name}")
}

/// The folder of every project's private files, the computer's: they hold secret values.
pub fn private_files() -> PathBuf {
  std::env::temp_dir().join("orchester-mcp")
}

/// The computer's folder of the apps' discovery files, in the user's data folder; `None` without one.
pub fn app_discovery() -> Option<PathBuf> {
  user_data().map(|data| data.join(APP_DISCOVERY))
}

/// The computer's QA slots folder, in the user's data folder; `None` without one.
pub fn qa_slots() -> Option<PathBuf> {
  user_data().map(|data| data.join("orchester-qa-slots"))
}

/// The user's data folder, which holds the apps' discovery folders and the QA slots.
#[cfg(windows)]
fn user_data() -> Option<PathBuf> {
  std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn user_data() -> Option<PathBuf> {
  std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn user_data() -> Option<PathBuf> {
  None
}
