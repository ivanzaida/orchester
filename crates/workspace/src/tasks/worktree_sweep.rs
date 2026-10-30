//! The sweep after a worktree removal (`orchester_core::integration::sweep`): folders under `.orchester/worktrees/`
//! that git no longer lists and whose run ended go, each deletion logged. The runner sweeps too, at its start.

use std::path::Path;

use orchester_core::integration::sweep;
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::changes::GIT;

/// Sweeps the worktree folders of the repository at `repo_root` that no git worktree and no run of `db` that has not
/// ended owns. It only frees space: what cannot be read or deleted is logged, and the next sweep tries again.
pub(super) fn sweep_in(db: &ProjectDb, programs: &Programs, repo_root: &Path) {
  let owners = match db.worktree_folder_owners() {
    Ok(owners) => owners,
    Err(error) => {
      tracing::warn!(%error, "cannot read which runs own the worktree folders; none is swept");
      return;
    }
  };
  let ended = |name: &str| owners.get(name).copied().unwrap_or(false);
  match sweep(&programs.git_work(), (Path::new(GIT), repo_root), ended) {
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
