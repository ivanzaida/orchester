//! A run whose worktree removal is recorded but whose folder is still on disk (ORC-418): it is *not removed*, with the
//! folder and why as the removal recorded them, never *removed*. A folder found there with no record of why (a
//! removal from before Orchester kept one) is not removed as well, without a reason.

use std::path::Path;

use orchester_core::{id::RunId, integration::NotRemoved, store::WorktreeStore};
use orchester_store::{ProjectDb, StoreError};

use super::changes::run_worktree;

/// What the recorded removal of `run`'s worktrees left on disk in the repository at `repo_root`; `None` when none is
/// recorded or nothing stayed.
pub fn not_removed(db: &ProjectDb, repo_root: &Path, run: RunId) -> Result<Option<NotRemoved>, StoreError> {
  if db.worktree_removal(run)?.is_none() {
    return Ok(None);
  }
  if let Some(left) = db
    .worktree_not_removed(run)?
    .filter(|left| Path::new(&left.path).exists())
  {
    return Ok(Some(left));
  }
  let own = run_worktree(repo_root, run);
  Ok(own.is_dir().then(|| NotRemoved {
    path: own.to_string_lossy().replace('\\', "/"),
    reason: None,
  }))
}
