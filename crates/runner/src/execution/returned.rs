//! A person returns the work of a run that ended to one of its agent steps with a note (ORC-44,
//! [`orchester_core::run::Run::return_ended`]): its task ended with it, so the return is recorded on the stored run,
//! which then continues in a new task as any continued run does. Only the task's latest run is returned, only while
//! its worktree is there and its work was not merged; the store refuses a merge or a removal recorded meanwhile.

use orchester_core::{
  id::RunId,
  resource::StepKey,
  run::{ReviewNote, Run},
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{Scheduler, failed, idle, prepare};

impl Scheduler {
  /// Returns the work of `run`, which ended, to its agent step `step` with `note`, and runs it from there. A run a
  /// task still executes has not ended.
  pub async fn return_ended(&self, run: RunId, step: StepKey, note: ReviewNote) -> Result<(), Rejection> {
    let executing = |rejection| match rejection {
      Rejection::RunNotPaused => Rejection::RunNotEnded,
      other => other,
    };
    self
      .continue_settled(run, |db, root| {
        let mut stored = returnable(db, run)?;
        prepare::existing_worktree(root, run)?;
        let events = stored
          .return_ended(&step, &note, Timestamp::now())
          .map_err(idle::rejection)?;
        idle::store_continuation(db, root, stored, &events)
      })
      .await
      .map_err(executing)
  }
}

/// The stored run, if its work can be returned: the task's latest run, ended, and not merged.
fn returnable(db: &ProjectDb, run: RunId) -> Result<Run, Rejection> {
  let stored = idle::load(db, run)?;
  if !stored.can_return_ended() {
    return Err(Rejection::RunNotEnded);
  }
  let listed = db
    .runs_of_task(stored.task())
    .map_err(|error| failed("cannot read task runs", &error))?;
  if !matches!(listed.last(), Some(Ok(latest)) if latest.id() == run) {
    return Err(Rejection::RunNotEnded);
  }
  if db
    .run_merge(run)
    .map_err(|error| failed("cannot read the run's merge", &error))?
    .is_some()
  {
    return Err(Rejection::RunMerged);
  }
  Ok(stored)
}
