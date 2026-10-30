//! The sweep of `.orchester/worktrees/` (`orchester_core::integration::sweep`) at the runner's start and after each
//! removal of a run's worktrees: folders git no longer lists and whose run ended — what removals that failed part way
//! left, often a whole build's output — go, each deletion logged. It runs on a thread of its own, as deleting such a
//! folder can take minutes, and only frees space: what cannot be read or deleted is logged for the next sweep.

use std::{
  path::{Path, PathBuf},
  thread,
  time::Duration,
};

use orchester_core::integration::sweep;
use orchester_exec::ProcessRunner;
use orchester_store::ProjectDb;

/// How long git may take to list the worktrees.
const GIT_TIMEOUT: Duration = Duration::from_secs(60);

/// Sweeps the worktree folders of the project at `root` on a thread of its own.
pub(super) fn sweep_in_background(root: PathBuf) {
  let started = thread::Builder::new()
    .name("worktree-sweep".into())
    .spawn(move || sweep_now(&root));
  if let Err(error) = started {
    tracing::warn!(%error, "cannot start the sweep of worktree folders");
  }
}

fn sweep_now(root: &Path) {
  let owners = match ProjectDb::open(root).and_then(|db| db.worktree_folder_owners()) {
    Ok(owners) => owners,
    Err(error) => {
      tracing::warn!(%error, "cannot read which runs own the worktree folders; none is swept");
      return;
    }
  };
  let ended = |name: &str| owners.get(name).copied().unwrap_or(false);
  match sweep(&ProcessRunner::new(GIT_TIMEOUT), (Path::new("git"), root), ended) {
    Ok(swept) => {
      for folder in swept {
        match folder.outcome {
          Ok(()) => tracing::info!(path = %folder.path.display(), "deleted a worktree folder no run uses"),
          Err(error) => {
            tracing::warn!(path = %folder.path.display(), %error, "cannot delete a worktree folder no run uses");
          }
        }
      }
    }
    Err(error) => tracing::warn!(%error, "cannot sweep the worktree folders"),
  }
}
