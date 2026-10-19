//! A person returns the work of a run that ended to one of its agent steps with a note (the project task ORC-44 ·
//! Return a finished run to an earlier step; [`orchester_core::run::Run::return_ended`]): its task ended with it, so
//! the return is recorded on the stored run, which then continues in a new task as any continued run does. Only the
//! task's latest run is returned, only while its worktree is there and its work was not merged; the store refuses a
//! merge or a removal recorded meanwhile.

use orchester_core::{
  id::RunId,
  run::Run,
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::{Rejection, WorkReturn};
use orchester_store::ProjectDb;

use super::{Scheduler, failed, idle, prepare};

impl Scheduler {
  /// Returns the work of `returned.run`, which ended, to its agent step with the note, and runs it from there. A run
  /// a task still executes has not ended, and one whose worktree a merge waits in for a person does not continue.
  pub async fn return_ended(&self, returned: WorkReturn) -> Result<(), Rejection> {
    let WorkReturn {
      run,
      step,
      note,
      conflicts,
    } = returned;
    let executing = |rejection| match rejection {
      Rejection::RunNotPaused => Rejection::RunNotEnded,
      other => other,
    };
    self
      .continue_settled(run, |db, root| {
        let mut stored = returnable(db, run)?;
        let worktree = prepare::existing_worktree(root, run)?;
        in_merge(&worktree)?;
        let events = stored
          .return_ended(&step, (&note, conflicts.as_ref()), Timestamp::now())
          .map_err(idle::rejection)?;
        idle::store_continuation(db, root, stored, &events)
      })
      .await
      .map_err(executing)
  }
}

/// Refuses a return while a merge waits in the run's `worktree`, which a person may have begun outside Orchester; one
/// Orchester began is recorded, and the store refuses that too.
fn in_merge(worktree: &std::path::Path) -> Result<(), Rejection> {
  let merging = crate::git::Git::new(worktree)
    .succeeds(&["rev-parse", "--quiet", "--verify", "MERGE_HEAD"])
    .map_err(|error| failed("cannot tell whether a merge waits in the run's worktree", &error))?;
  if merging {
    Err(Rejection::WorktreeInMerge)
  } else {
    Ok(())
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
