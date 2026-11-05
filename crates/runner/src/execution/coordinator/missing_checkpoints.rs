//! Succeeded runs whose agents committed their work themselves recorded no checkpoint before ORC-147, so whatever reads
//! a run's work through its checkpoints (its Changes, `get_changes`) found none. As the runner starts, each such run
//! whose branch is ahead of the revision it started from, as the executor reads the runs' branches
//! (`super::super::executor::run_branches`), records the branch's tip as its checkpoint, once.

use std::path::Path;

use orchester_core::{
  revision::Revision,
  run::{Run, RunError},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use crate::{execution::executor::run_branches::BranchTips, worktree::WorktreeError};

#[derive(Debug, thiserror::Error)]
enum RepairError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Runs(#[from] RunStoreError),
  #[error(transparent)]
  Run(#[from] RunError),
  #[error(transparent)]
  Git(#[from] WorktreeError),
}

/// Records the missing checkpoints of the project at `root`; what cannot be read or recorded is logged, and its run
/// is tried again when the runner starts next.
pub(in crate::execution) fn record(root: &Path) {
  if let Err(error) = record_in(root) {
    tracing::warn!(%error, "cannot record the checkpoints succeeded runs lack");
  }
}

fn record_in(root: &Path) -> Result<(), RepairError> {
  let mut db = ProjectDb::open(root)?;
  let runs = db.succeeded_runs_without_checkpoint()?;
  if runs.is_empty() {
    return Ok(());
  }
  let tips = BranchTips::read(root)?;
  for run in runs {
    let run = match run {
      Ok(run) => run,
      Err(unreadable) => {
        tracing::warn!(run = %unreadable.id, error = %unreadable.error, "a succeeded run cannot be read");
        continue;
      }
    };
    let Some(tip) = tips.ahead_of(run.branch(), run.base())?.cloned() else {
      continue;
    };
    let id = run.id();
    match record_tip(&mut db, run, tip.clone()) {
      Ok(()) => tracing::info!(run = %id, %tip, "recorded the checkpoint of a run whose agents committed themselves"),
      Err(error) => tracing::warn!(run = %id, %error, "cannot record the checkpoint of a succeeded run"),
    }
  }
  Ok(())
}

fn record_tip(db: &mut ProjectDb, mut run: Run, tip: Revision) -> Result<(), RepairError> {
  let events = run.record_missing_checkpoint(tip, Timestamp::now())?;
  db.append_events(&run, &events)?;
  Ok(())
}
