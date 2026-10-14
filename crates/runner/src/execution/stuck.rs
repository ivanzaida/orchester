//! A person's decision on a run that waits for them after its retries ran out, its step broke again, reported
//! unverified or was not run again on an unchanged revision ([`orchester_core::run::StuckDecision`]). Its task ended
//! when it began to wait, so the decision is carried out on the stored run: one more retry, the work returned to an
//! agent step, or an accepted step continues the run in a new task as any continued run does — the accepted step
//! passes there as a step passes (`step::accepted`); a stop fails it.

use orchester_core::{
  id::RunId,
  run::{Run, RunStatus, StuckDecision},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;

use super::{Scheduler, idle};

impl Scheduler {
  /// Carries out `decision` on `run`, which waits for a person. A run its task executes does not
  /// wait for one: a second decision while the first one's attempt runs is refused as such.
  pub async fn decide_stuck(&self, run: RunId, decision: StuckDecision) -> Result<(), Rejection> {
    let executing = |rejection| match rejection {
      Rejection::RunNotPaused => Rejection::RunNotWaiting,
      other => other,
    };
    if decision != StuckDecision::Stop {
      return self
        .continue_settled(run, |db, root| {
          let mut stored = waiting(db, run)?;
          if matches!(decision, StuckDecision::Accept { .. }) {
            acceptable(root, &stored)?;
          }
          let events = stored
            .decide_stuck(&decision, Timestamp::now())
            .map_err(idle::rejection)?;
          idle::store_continuation(db, root, stored, &events)
        })
        .await
        .map_err(executing);
    }
    self.settled(run).await.map_err(executing)?;
    let _one_at_a_time = self.inner.continuing.lock().await;
    let mut db = self.open()?;
    let mut stored = waiting(&db, run)?;
    let events = stored
      .decide_stuck(&decision, Timestamp::now())
      .map_err(idle::rejection)?;
    idle::store(&mut db, &stored, &events)?;
    self.inner.notifier.send(Event::RunChanged {
      run,
      task: stored.task(),
    });
    Ok(())
  }
}

/// Refuses accepting `run`'s waiting step when the work that would pass with it is not the work its result was for
/// (`Workspace::accepted_work`): the person reads why.
fn acceptable(root: &std::path::Path, run: &Run) -> Result<(), Rejection> {
  let accepted = run.acceptable().ok_or(Rejection::RunNotWaiting)?;
  let run_worktree = crate::worktree::Worktrees::new(root)
    .map(|worktrees| worktrees.path(run.id()))
    .map_err(|error| Rejection::Failed {
      detail: error.to_string(),
    })?;
  super::workspace::Workspace::accepted_work(root, (run, &run_worktree), accepted)
    .map(drop)
    .map_err(|detail| Rejection::Failed { detail })
}

/// The stored run, if it waits for a person.
fn waiting(db: &ProjectDb, run: RunId) -> Result<Run, Rejection> {
  let stored = idle::load(db, run)?;
  if matches!(stored.status(), RunStatus::WaitingForPerson(_)) {
    Ok(stored)
  } else {
    Err(Rejection::RunNotWaiting)
  }
}
