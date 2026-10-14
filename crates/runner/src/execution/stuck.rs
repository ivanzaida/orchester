//! A person's decision on a run that waits for them after its retries ran out, its step broke again, reported
//! unverified or was not run again on an unchanged revision ([`orchester_core::run::StuckDecision`]). Its task ended
//! when it began to wait, so the decision is carried out on the stored run: one more retry, the work returned to an
//! agent step, or an accepted step that leaves steps to run continues the run in a new task as any continued run does;
//! a stop fails it, and an accepted last step succeeds it.

use orchester_core::{
  id::RunId,
  run::{Run, RunStatus, StuckDecision},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;

use super::{Scheduler, idle, record::announce_ready};

impl Scheduler {
  /// Carries out `decision` on `run`, which waits for a person. A run its task executes does not
  /// wait for one: a second decision while the first one's attempt runs is refused as such.
  pub async fn decide_stuck(&self, run: RunId, decision: StuckDecision) -> Result<(), Rejection> {
    let executing = |rejection| match rejection {
      Rejection::RunNotPaused => Rejection::RunNotWaiting,
      other => other,
    };
    if !self.ends(run, &decision)? {
      return self
        .continue_settled(run, |db, root| {
          let mut stored = waiting(db, run)?;
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
    announce_ready(&self.inner.notifier, &stored);
    Ok(())
  }

  /// Whether `decision` ends the run instead of continuing it: a stop, or the acceptance of a step after which every
  /// step has passed.
  fn ends(&self, run: RunId, decision: &StuckDecision) -> Result<bool, Rejection> {
    match decision {
      StuckDecision::Stop => Ok(true),
      StuckDecision::Accept => {
        let mut probe = waiting(&self.open()?, run)?;
        probe
          .decide_stuck(decision, Timestamp::now())
          .map_err(idle::rejection)?;
        Ok(probe.status().is_terminal())
      }
      StuckDecision::Retry | StuckDecision::ReturnTo { .. } => Ok(false),
    }
  }
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
