//! The coordinator's side of [`orchester_ipc::Command::RunFolderState`]: which folder is the run's, from the project
//! database — its own worktree from before workspaces, else the workspace it holds (ORC-114), else the one it worked
//! in last while no other run took it since — handed to the executor, which reads the folder's state with git.

use std::path::{Path, PathBuf};

use orchester_core::{id::RunId, integration::WORKTREES_DIR, store::WorkspaceStore, workspaces::Holder};
use orchester_ipc::{FolderState, Reply};
use orchester_store::ProjectDb;

use super::{super::executor::folder_state, Scheduler};

impl Scheduler {
  /// The answer to [`orchester_ipc::Command::RunFolderState`]: what `run`'s folder holds beyond its last commit, read
  /// off the async workers; unknown when it has no folder any more or git cannot tell.
  pub async fn run_folder_state(&self, run: RunId) -> Reply {
    let root = self.inner.project_root.clone();
    let state = tokio::task::spawn_blocking(move || folder_state::read(&folders(&root, run)))
      .await
      .unwrap_or_else(|error| {
        tracing::error!(%run, %error, "reading the run's folder failed");
        FolderState::Unknown
      });
    Reply::RunFolderState { run, state }
  }
}

/// Where `run` of the repository at `root` may work, in the order its folder is looked for: its own worktree, then the
/// workspace it holds or held last. A database that cannot be read leaves only its own worktree, and is logged.
fn folders(root: &Path, run: RunId) -> Vec<PathBuf> {
  let mut folders = vec![root.join(WORKTREES_DIR).join(run.to_string())];
  let records = ProjectDb::open_read_only(root).and_then(|db| db.workspaces());
  match records {
    Ok(records) => {
      let held = records
        .iter()
        .find(|record| record.holder == Some(Holder::Run { run }))
        .or_else(|| {
          records
            .iter()
            .find(|record| record.holder.is_none() && record.last_run == Some(run))
        });
      folders.extend(held.map(|record| record.number.path(root)));
    }
    Err(error) => tracing::warn!(%run, %error, "the workspaces cannot be read"),
  }
  folders
}
