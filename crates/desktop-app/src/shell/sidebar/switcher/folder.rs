//! *Open folder…* (`Project switcher · Open folder · checking and refusals` `uK2Hw`): a folder that keeps
//! an Orchester project opens it, and one already in the list simply opens; any other folder is refused
//! with why — it is in no git repository, it is inside one but not its top folder, it is a repository
//! Orchester has not set up, its project data was written by a newer Orchester, or it cannot be read.
//! Nothing is added to the list then.

use std::path::{Path, PathBuf};

use orchester_store::StoreError;
use orchester_workspace::{
  messages::{self, Message},
  workspace::{OpenProject, Workspace},
};

/// Why a chosen folder does not open as a project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderRefusal {
  /// No git repository holds the folder.
  NotRepository,
  /// A repository holds the folder, but it starts at `top`.
  NotTopFolder { top: PathBuf },
  /// A git repository Orchester has not set up.
  NoProject,
  /// Its project data has version `found`; this build reads up to `supported`.
  Newer { found: u32, supported: u32 },
  /// It cannot be read, in the system's words.
  Unreadable(Message),
}

impl lurq::app::component::DevtoolsInspectable for FolderRefusal {}

/// Opens the project kept in `folder` and makes it the most recently used one, or says why it cannot.
/// Blocks on SQLite and the file system.
pub fn open_folder(workspace: &Workspace, folder: &Path) -> Result<OpenProject, FolderRefusal> {
  workspace.open(folder).map_err(|error| refusal(folder, &error))
}

fn refusal(folder: &Path, error: &StoreError) -> FolderRefusal {
  match error {
    StoreError::NotInitialized(_) | StoreError::NotAGitRepository(_) => {
      if holds_git(folder) {
        return FolderRefusal::NoProject;
      }
      match folder.ancestors().skip(1).find(|above| holds_git(above)) {
        Some(top) => FolderRefusal::NotTopFolder { top: top.to_path_buf() },
        None => FolderRefusal::NotRepository,
      }
    }
    StoreError::SchemaTooNew { found, supported } => FolderRefusal::Newer {
      found: *found,
      supported: *supported,
    },
    other => {
      tracing::warn!(error = %other, folder = %folder.display(), "cannot open the chosen folder");
      FolderRefusal::Unreadable(messages::store_error(other))
    }
  }
}

/// Whether `folder` is the top of a git repository: it holds `.git`, a folder or, in a worktree, a file.
fn holds_git(folder: &Path) -> bool {
  folder.join(".git").exists()
}
