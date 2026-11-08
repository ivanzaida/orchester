//! Where local integration and the direct merge (*Merge into main*, `merge_run`) meet: which local integration holds
//! the shared checkout, so the direct merge waits for it, and the run's own review once the run's merge was recorded
//! beside it, or git proves its work merged without Orchester.

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::{
    RunMerge,
    local::{
      ContainedResult, LocalError, LocalIntegrationStore, LocalOperation, LocalPhase, LocalProcessStore,
      ResultEvidenceKind,
    },
  },
  project::TaskKey,
  store::{RunStore, TaskStore},
};
use orchester_exec::local_git::IntegrationLease;
use orchester_store::{ProjectDb, StoreError};

use super::{IntegrationError, settle::wait_while};
use crate::tasks::detail::run_store_error;

/// The local integration that holds the shared checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckoutHolder {
  /// The run's own.
  ThisRun,
  /// A Git program of the run's own local integration is journaled as running, and the integration holds nothing
  /// else: there is nothing for a person to finish in Local integration, only the program to end.
  RunningProgram,
  /// Another run's, of the task `task`.
  OtherRun { task: TaskKey },
}

/// Which local integration holds the shared checkout, if any: one that started work it must finish or put back
/// (`LocalOperation::reserves_checkout`), or whose Git process has not finished, as local integration itself checks
/// before it begins. A review that only read the checkout, also one paused since, holds nothing and does not stand in
/// the way of the run's direct merge; historical finished records do not reserve the checkout forever.
///
/// A run's own Git program that is journaled as running is given a moment to end first, as it does when a merge is
/// asked the moment the run's last program is closing; one that is really running outlasts the wait.
pub(crate) fn checkout_holder(db: &ProjectDb, run: RunId) -> Result<Option<CheckoutHolder>, StoreError> {
  wait_while(
    || holder_now(db, run),
    |holder| matches!(holder, Ok(Some(CheckoutHolder::RunningProgram))),
  )
}

fn holder_now(db: &ProjectDb, run: RunId) -> Result<Option<CheckoutHolder>, StoreError> {
  let holders = holders(db)?;
  if holders.contains(&run) {
    let reserves = db
      .local_operation(run)?
      .is_some_and(|operation| operation.reserves_checkout());
    return Ok(Some(if reserves {
      CheckoutHolder::ThisRun
    } else {
      CheckoutHolder::RunningProgram
    }));
  }
  let Some(other) = holders.first() else {
    return Ok(None);
  };
  Ok(Some(CheckoutHolder::OtherRun {
    task: task_of(db, *other)?,
  }))
}

/// Whether any local integration holds the shared checkout, as [`checkout_holder`] reads it: a merge that is no run's,
/// a team module's delivery, waits for it.
pub(crate) fn checkout_held(db: &ProjectDb) -> Result<bool, StoreError> {
  Ok(!holders(db)?.is_empty())
}

/// The runs whose local integration holds the shared checkout.
fn holders(db: &ProjectDb) -> Result<Vec<RunId>, StoreError> {
  let mut holders = Vec::new();
  for operation in db.local_operations()? {
    let unfinished = db
      .local_process(operation.run())?
      .is_some_and(|process| !process.finished);
    if operation.reserves_checkout() || unfinished {
      holders.push(operation.run());
    }
  }
  Ok(holders)
}

/// The key of the task of run `run`, which holds a local integration record and so is stored.
fn task_of(db: &ProjectDb, run: RunId) -> Result<TaskKey, StoreError> {
  let run = db
    .run(run)
    .map_err(run_store_error)?
    .ok_or(StoreError::RunNotFound(run))?;
  let task = db.task(run.task())?.ok_or(StoreError::TaskNotFound(run.task()))?;
  Ok(task.key().clone())
}

/// Marks the run's own local integration merged once the direct merge recorded `merge`, where it held nothing of the
/// checkout: its tab then shows the merge instead of the review or cancellation it last showed. Opening the tab
/// reconciles the same way, so a record that cannot be settled now (the store refused, or another window changed it)
/// costs nothing but a log line; the merge stands either way.
pub(crate) fn settle_review(db: &mut ProjectDb, merge: &RunMerge) {
  if let Err(error) = settle(db, merge) {
    tracing::warn!(%error, "the run was merged, but its local integration record could not be marked merged");
  }
}

fn settle(db: &mut ProjectDb, merge: &RunMerge) -> Result<(), IntegrationError> {
  let Some(mut operation) = db.local_operation(merge.run)? else {
    return Ok(());
  };
  if operation.reserves_checkout() || operation.merged().is_some() {
    return Ok(());
  }
  let version = operation.version();
  operation.reconcile_recorded_merge(merge)?;
  operation.advance_version();
  if !db.save_local_operation(&operation, Some(version))? {
    return Err(LocalError::Stale.into());
  }
  Ok(())
}

/// Completes `operation`, the run's own local integration as `db` holds it, where it holds nothing of the checkout
/// and `evidence` proves the run's whole work merged without Orchester (`LocalOperation::settle_merged_outside`):
/// its tab and the project's other runs then read it as done instead of paused at a review. Only while no local
/// integration works in the project at `root` and no Git process of this one is unfinished; otherwise, and when the
/// store refuses or another window changed the record, a later reading settles it, so this costs a log line at most.
pub(super) fn settle_outside(
  root: &Path,
  db: &mut ProjectDb,
  operation: &mut LocalOperation,
  evidence: &ContainedResult,
) {
  if evidence.kind != ResultEvidenceKind::CompleteAncestry
    || operation.phase() == LocalPhase::Complete
    || operation.merged().is_some()
    || operation.reserves_checkout()
  {
    return;
  }
  // Another local integration works in the checkout now; its session holds what it read of the records.
  let Ok(_lease) = IntegrationLease::acquire(root) else {
    return;
  };
  match settled_outside(db, operation, evidence) {
    Ok(Some(settled)) => *operation = settled,
    Ok(None) => {}
    Err(error) => {
      tracing::warn!(%error, "the run's work is merged, but its local integration record could not be completed")
    }
  }
}

fn settled_outside(
  db: &mut ProjectDb,
  operation: &LocalOperation,
  evidence: &ContainedResult,
) -> Result<Option<LocalOperation>, IntegrationError> {
  if db
    .local_process(operation.run())?
    .is_some_and(|process| !process.finished)
  {
    return Ok(None);
  }
  let mut settled = operation.clone();
  let version = settled.version();
  settled.settle_merged_outside(evidence)?;
  settled.advance_version();
  Ok(db.save_local_operation(&settled, Some(version))?.then_some(settled))
}
