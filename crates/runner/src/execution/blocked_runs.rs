//! Runs that people's messages wait for while their task is blocked: the answer or follow-up is
//! stored, but the run cannot go on until the task's linked blocking tasks finish. The scheduler's
//! pass delivers them then, without another message. Only a blocking task that runs keeps the
//! runner busy for them; one that waits for a person or a merge does not.

use std::collections::HashSet;

use orchester_core::{id::RunId, run::RunAgainBlock, store::RunStore};
use orchester_ipc::Rejection;
use parking_lot::Mutex;

use super::{Scheduler, failed, read_failed, task_groups::blocker_runs};

/// The runs whose queued messages wait for their task's blocking tasks.
#[derive(Default)]
pub(super) struct BlockedRuns(Mutex<HashSet<RunId>>);

/// Whether `rejection` is the refusal of a run whose task is blocked.
pub(super) const fn is_blocked(rejection: &Rejection) -> bool {
  matches!(
    rejection,
    Rejection::RunAgainBlocked {
      block: RunAgainBlock::TaskBlocked
    }
  )
}

impl Scheduler {
  /// Remembers that `run`'s queued messages wait for its task's blocking tasks.
  pub(super) fn hold_for_blockers(&self, run: RunId) {
    self.inner.blocked_runs.0.lock().insert(run);
  }

  /// Delivers the queued messages of each remembered run whose task is no longer blocked, as a
  /// new message would; a run that is still blocked stays remembered.
  pub(super) async fn release_unblocked(&self) -> Result<(), Rejection> {
    let runs: Vec<RunId> = self.inner.blocked_runs.0.lock().iter().copied().collect();
    if runs.is_empty() {
      return Ok(());
    }
    let _one_at_a_time = self.inner.continuing.lock().await;
    let db = self.open()?;
    for run in runs {
      let Some(stored) = db
        .run(run)
        .map_err(|error| read_failed("cannot read a blocked run", error))?
      else {
        self.inner.blocked_runs.0.lock().remove(&run);
        continue;
      };
      let blocking = db
        .unfinished_blocking_tasks(stored.task())
        .map_err(|error| failed("cannot read blocking tasks", &error))?;
      if !blocking.is_empty() {
        continue;
      }
      self.inner.blocked_runs.0.lock().remove(&run);
      if self.is_active(run) {
        continue;
      }
      tracing::info!(%run, "the blocking tasks finished: the queued messages are delivered");
      if let Err(rejection) = self.deliver_idle(run) {
        tracing::warn!(%run, ?rejection, "queued messages cannot be delivered; they stay queued");
      }
    }
    Ok(())
  }

  /// Whether a remembered run waits for a blocking task that runs now, which will finish without a
  /// person.
  pub(super) fn blocked_runs_wait_for_work(&self) -> Result<bool, Rejection> {
    let runs: Vec<RunId> = self.inner.blocked_runs.0.lock().iter().copied().collect();
    if runs.is_empty() {
      return Ok(false);
    }
    let db = self.open()?;
    for run in runs {
      let Some(stored) = db
        .run(run)
        .map_err(|error| read_failed("cannot read a blocked run", error))?
      else {
        continue;
      };
      let blocking = db
        .unfinished_blocking_tasks(stored.task())
        .map_err(|error| failed("cannot read blocking tasks", &error))?;
      if blocker_runs(&db, &blocking)? {
        return Ok(true);
      }
    }
    Ok(false)
  }
}
