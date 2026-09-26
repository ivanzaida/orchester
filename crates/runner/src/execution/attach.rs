//! Recovering runs when the desktop attaches (`docs/architecture/storage.md`, "Lifecycle"): runs
//! still recorded as running were cut off, since a new runner executes nothing yet. They are
//! recorded as interrupted, with a cause that depends on the session: the previous runner's loss
//! (they continue on their own), or the end of the previous session (they wait for a person).

use std::num::NonZeroU32;

use orchester_core::{
  connection::stream::InvocationOutcome,
  id::RunId,
  run::{InterruptCause, Run},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Event, RefusedRun, Rejection};
use orchester_store::ProjectDb;

use super::{Scheduler, failed, idle};

/// What attaching did to the runs it found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attached {
  /// Runs recorded as interrupted now.
  pub interrupted: Vec<RunId>,
  /// Interrupted runs continued because the previous runner dropped.
  pub continued: Vec<RunId>,
}

impl Scheduler {
  /// The desktop attached for `session` with `slots` agent slots: records the runs left running
  /// as interrupted and, after the previous runner dropped, continues the runs its loss
  /// interrupted, as far as slots and the daily budget allow.
  pub fn attach(&self, session: AttachSession, slots: NonZeroU32) -> Result<Attached, Rejection> {
    self.inner.board.set_slots(slots);
    let mut db = self.open()?;
    let cause = session.interrupt_cause();
    let stale = db
      .running_runs()
      .map_err(|error| failed("cannot read the running runs", &error))?;
    let mut attached = Attached::default();
    for mut run in stale.into_iter().filter(|run| !self.is_active(run.id())) {
      interrupt(&mut db, &mut run, cause)?;
      self.announce(&run);
      attached.interrupted.push(run.id());
    }
    if session == AttachSession::Reconnect {
      attached.continued = self.continue_after_reconnect(&db)?;
    }
    self.inner.board.announce();
    Ok(attached)
  }

  /// Continues interrupted runs `runs` from their last checkpoints because a person asked to.
  pub fn continue_runs(&self, runs: &[RunId]) -> (Vec<RunId>, Vec<RefusedRun>) {
    let mut continued = Vec::new();
    let mut refused = Vec::new();
    for &run in runs {
      match self.continue_idle(run, |db, root| idle::continue_interrupted(db, root, run)) {
        Ok(()) => continued.push(run),
        Err(rejection) => refused.push(RefusedRun { run, rejection }),
      }
    }
    (continued, refused)
  }

  fn continue_after_reconnect(&self, db: &ProjectDb) -> Result<Vec<RunId>, Rejection> {
    let waiting = db
      .interrupted_runs()
      .map_err(|error| failed("cannot read the interrupted runs", &error))?;
    let mut continued = Vec::new();
    for run in waiting.iter().filter(|run| run.continues_on_reconnect()) {
      let id = run.id();
      match self.continue_idle(id, |db, root| idle::continue_after_reconnect(db, root, id)) {
        Ok(()) => continued.push(id),
        // It stays interrupted and can be continued from its task page once there is room.
        Err(rejection) => tracing::warn!(run = %id, ?rejection, "an interrupted run cannot continue yet"),
      }
    }
    Ok(continued)
  }

  fn announce(&self, run: &Run) {
    self.inner.notifier.send(Event::RunChanged {
      run: run.id(),
      task: run.task(),
    });
  }
}

/// Records `run`, left running by a runner that is gone, as interrupted by `cause`. Its agent
/// ended with that runner, so an invocation still open is closed at its last transcript entry:
/// active time must not keep growing for an agent that no longer runs.
fn interrupt(db: &mut ProjectDb, run: &mut Run, cause: InterruptCause) -> Result<(), Rejection> {
  let store = |error: &dyn std::error::Error| failed("cannot record the interrupted run", error);
  let activity = db.activity(run.id(), 0).map_err(|error| store(&error))?;
  for mut invocation in db.invocations(run.id()).map_err(|error| store(&error))? {
    if invocation.ended_at.is_some() {
      continue;
    }
    let last_seen = activity
      .iter()
      .rev()
      .find(|entry| entry.invocation == invocation.id)
      .map_or(invocation.started_at, |entry| entry.at);
    invocation.ended_at = Some(last_seen);
    invocation.outcome = Some(InvocationOutcome::Terminated);
    db.save_invocation(&invocation).map_err(|error| store(&error))?;
  }
  let events = run.interrupt(cause, Timestamp::now()).map_err(|error| store(&error))?;
  db.append_events(run, &events).map_err(|error| store(&error))
}
