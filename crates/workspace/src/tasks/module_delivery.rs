//! A team module's branch merged into its base on this computer (team authority P4c): core's module merge
//! ([`merge_module`]), the merge a run takes into its base branch, made under the project's merge hold so local
//! integration and every other merge stay out of the checkout meanwhile, and only while no local integration holds the
//! checkout with work it must finish. A merge whose record was lost is found again ([`module_merged_in`]), never made
//! twice. Blocks on SQLite and git.

use std::path::Path;

use orchester_core::{
  integration::{GitError, MergeError, ModuleDelivery, ModuleMerge, merge_module, module_merged},
  revision::Revision,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::{changes::GIT, integration::MergeHold, local_integration::checkout_held};

/// Why a module delivery did not merge.
#[derive(Debug, thiserror::Error)]
pub enum ModuleMergeError {
  /// Another merge or a local integration holds the checkout now: nothing was merged, and the merge is tried again.
  #[error("another merge or a local integration holds the checkout")]
  Busy,
  /// Git did not merge it: the branch or the base moved, the checkout stood in the way, or the merge conflicted.
  #[error(transparent)]
  Merge(#[from] MergeError),
  #[error(transparent)]
  Store(#[from] StoreError),
}

/// Merges `delivery` in the repository at `repo_root`, then tells the runs whose base branch moved.
pub fn merge_module_in(
  repo_root: &Path,
  programs: &Programs,
  delivery: &ModuleDelivery<'_>,
) -> Result<ModuleMerge, ModuleMergeError> {
  let Ok(_hold) = MergeHold::acquire(repo_root) else {
    return Err(ModuleMergeError::Busy);
  };
  if checkout_held(&ProjectDb::open(repo_root)?)? {
    return Err(ModuleMergeError::Busy);
  }
  let merged = merge_module(&programs.git_work(), (Path::new(GIT), repo_root), delivery)?;
  if let Some(left) = &merged.left {
    tracing::warn!(path = %left.path, detail = %left.detail, "delivered, but the temporary worktree of the merge commit stayed");
  }
  crate::base_moved::after_merge(repo_root, programs);
  Ok(merged)
}

/// The merge commit an earlier [`merge_module_in`] of `delivery` left on its base branch, if it is still its tip.
pub fn module_merged_in(
  repo_root: &Path,
  programs: &Programs,
  delivery: &ModuleDelivery<'_>,
) -> Result<Option<Revision>, GitError> {
  module_merged(&programs.git_work(), (Path::new(GIT), repo_root), delivery)
}
