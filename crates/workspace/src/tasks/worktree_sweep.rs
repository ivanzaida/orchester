//! The sweep after a worktree removal (`ProjectDb::sweep_worktree_folders`): folders under `.orchester/worktrees/`
//! that git no longer lists and whose run ended go, each deletion logged. It runs on a thread of its own, as deleting
//! such a folder can take minutes and a merge or a removal does not wait for other runs' folders. The runner sweeps
//! too, at its start.

use std::{
  path::{Path, PathBuf},
  thread,
};

use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::changes::GIT;

/// Sweeps the worktree folders of the repository at `repo_root` that no git worktree and no run that has not ended
/// owns, on a thread of its own.
pub(super) fn sweep_in_background(programs: &Programs, repo_root: &Path) {
  let (programs, root): (Programs, PathBuf) = (programs.clone(), repo_root.to_path_buf());
  let started = thread::Builder::new()
    .name("worktree-sweep".into())
    .spawn(move || match ProjectDb::open(&root) {
      Ok(db) => db.sweep_worktree_folders(&programs.git_work(), (Path::new(GIT), &root)),
      Err(error) => tracing::warn!(%error, "cannot read which runs own the worktree folders; none is swept"),
    });
  if let Err(error) = started {
    tracing::warn!(%error, "cannot start the sweep of worktree folders");
  }
}
