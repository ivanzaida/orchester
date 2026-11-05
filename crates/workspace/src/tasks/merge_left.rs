//! What a merge that did not end as it should left in the person's checkout (`crates/desktop-app/design/orchester.pen`,
//! board `Review · redesign`, 09l `Asu5i`). A bring-in that stopped part way and could not put back every file it
//! wrote leaves its run *Stopped while bringing the merge in*, with those files; *Put files back* runs the put-back
//! again, never touching a file the person changed since, and no merge reaches the checkout until they hold their
//! earlier version again (`orchester_core::integration::put_back`). A merge the app or the runner ended part way is
//! found when the app starts again ([`recover_interrupted`]). Blocks on SQLite and git.

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::{left_by_bring_in, put_back},
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::{
  changes::GIT,
  integration::{IntegrationProblem, hold},
  merge_attempts::settle_interrupted,
};

/// The files a bring-in of `run`'s merge left changed in the checkout at `root` and that are not put back yet; empty
/// when it left none.
pub fn left_changed(root: &Path, programs: &Programs, run: RunId) -> Result<Vec<String>, IntegrationProblem> {
  left_by_bring_in(&programs.merge_git(), (Path::new(GIT), root), run).map_err(IntegrationProblem::Merge)
}

/// *Put files back*: puts back what a bring-in of `run`'s merge left changed in the checkout at `root`, holding what a
/// merge holds meanwhile. The files that still do not hold their earlier version, e.g. ones the person changed since,
/// as `MergeError::LeftChanged`.
pub fn put_files_back(root: &Path, programs: &Programs, run: RunId) -> Result<(), IntegrationProblem> {
  let _held = hold(root)?;
  put_back(&programs.merge_git(), (Path::new(GIT), root), run).map_err(|error| {
    tracing::warn!(%error, %run, "the files a merge left changed were not all put back");
    IntegrationProblem::Merge(error)
  })
}

/// Finds the merges in the project at `root` that the app or the runner ended part way, when the app starts: each one's
/// temporary worktree is removed, the files its bring-in left changed stay named for *Put files back*, and its attempt
/// is recorded as interrupted at its stage. While another merge or local integration holds the project, that one does
/// this instead. The runs whose attempts were recorded.
pub fn recover_interrupted(root: &Path, programs: &Programs) -> Result<Vec<RunId>, IntegrationProblem> {
  let _held = hold(root)?;
  let mut db = ProjectDb::open(root)?;
  Ok(settle_interrupted(&mut db, (root, programs))?)
}
