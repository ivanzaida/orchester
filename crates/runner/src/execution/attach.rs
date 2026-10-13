//! Recovering runs when the desktop attaches (`docs/architecture/storage.md`, "Lifecycle"): runs
//! still recorded as running were cut off, since a new runner executes nothing yet. They are
//! recorded as interrupted, with a cause that depends on the session: the previous runner's loss
//! (they continue on their own), or the end of the previous session (they wait for a person). A run
//! that only waits for a person's decision had no agent at work: it still waits, untouched, unless
//! people's messages written meanwhile answer its agent's question. A run that succeeded while
//! messages to its agents were written is followed up. A run whose record cannot be read back is left
//! as it is: it is logged and nothing continues it. A tool request still waiting in a run this runner
//! does not execute lost its agent: it ends without an answer.

use std::num::NonZeroU32;

use orchester_core::{
  connection::stream::InvocationOutcome,
  id::RunId,
  run::{InterruptCause, ListedRun, Run},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Event, RefusedRun, Rejection};
use orchester_store::{ProjectDb, RunStoreError};

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
  /// as interrupted, delivers the messages people wrote while no runner was attached (answers and
  /// follow-ups) and, after the previous runner dropped, continues the runs its loss interrupted, as
  /// far as slots and the daily budget allow.
  pub async fn attach(&self, session: AttachSession, slots: NonZeroU32) -> Result<Attached, Rejection> {
    let _one_at_a_time = self.inner.continuing.lock().await;
    if self.inner.board.is_stopping() {
      return Err(Rejection::ShuttingDown);
    }
    let mut db = self.open()?;
    if matches!(session, AttachSession::Fresh { .. }) {
      self.hold_groups_after_session_end(&db)?;
    }
    self.inner.board.set_slots(slots);
    let cause = session.interrupt_cause();
    let stale = readable(
      db.running_runs()
        .map_err(|error| failed("cannot read the running runs", &error))?,
      "running",
    );
    let mut attached = Attached::default();
    let cut_off = |run: &Run| !self.is_active(run.id()) && !run.waits_for_a_person();
    let (stale, waiting): (Vec<Run>, Vec<Run>) = stale.into_iter().partition(cut_off);
    for mut run in stale {
      interrupt(&mut db, &mut run, cause)?;
      self.announce(&run);
      attached.interrupted.push(run.id());
    }
    self.end_unheld_approvals(&mut db)?;
    self.end_unheld_questions(&mut db)?;
    self.wake_answered(&db, waiting)?;
    self.follow_up_queued(&mut db)?;
    if session == AttachSession::Reconnect {
      attached.continued = self.continue_after_reconnect(&db)?;
    }
    self.wait_for_plan_resets();
    self.inner.board.announce();
    Ok(attached)
  }

  /// The desktop changed how many agents may work at once to `slots`: it applies now. Runs waiting
  /// for a slot look again, and queued task groups try to start their tasks; agents already at work
  /// above a lowered count finish, and no new one starts until fewer work than `slots`.
  pub async fn set_agent_slots(&self, slots: NonZeroU32) -> Result<(), Rejection> {
    if self.inner.board.is_stopping() {
      return Err(Rejection::ShuttingDown);
    }
    self.inner.board.set_slots(slots);
    // The slots are set either way; groups that cannot advance now go on at the next pass.
    if let Err(error) = self.advance_task_groups().await {
      tracing::error!(
        ?error,
        "cannot advance reviewed task groups after the agent slots changed"
      );
    }
    Ok(())
  }

  /// Continues interrupted runs `runs` from their last checkpoints because a person asked to.
  pub async fn continue_runs(&self, runs: &[RunId]) -> (Vec<RunId>, Vec<RefusedRun>) {
    let _one_at_a_time = self.inner.continuing.lock().await;
    let mut continued = Vec::new();
    let mut refused = Vec::new();
    for &run in runs {
      match self.continue_idle(run, |db, root| idle::continue_interrupted(db, root, run)) {
        Ok(()) => {
          // The run continues whatever happens to its group's hold: a hold that could not be
          // released keeps only the group's other tasks waiting, and Start run releases it later.
          if let Err(error) = self.release_hold_of_continued(run) {
            tracing::error!(?error, %run, "continued run left its group's session hold in place");
          }
          continued.push(run);
        }
        Err(rejection) => refused.push(RefusedRun { run, rejection }),
      }
    }
    (continued, refused)
  }

  fn release_hold_of_continued(&self, run: RunId) -> Result<(), Rejection> {
    let db = self.open()?;
    let saved = db
      .run(run)
      .map_err(|e| super::read_failed("cannot read continued run", e))?
      .ok_or(Rejection::RunNotFound)?;
    if db
      .task_group_for_run(&saved)
      .map_err(|e| super::read_failed("cannot read continued group", e))?
      .is_some()
    {
      self.release_group_session_hold(&db, saved.task())?;
    }
    Ok(())
  }

  fn continue_after_reconnect(&self, db: &ProjectDb) -> Result<Vec<RunId>, Rejection> {
    let waiting = readable(
      db.interrupted_runs()
        .map_err(|error| failed("cannot read the interrupted runs", &error))?,
      "interrupted",
    );
    let mut continued = Vec::new();
    for run in waiting.iter().filter(|run| run.continues_on_reconnect()) {
      let id = run.id();
      if db
        .has_interrupted_tool_approval(id)
        .map_err(|error| failed("cannot inspect interrupted tool approvals", &error))?
      {
        tracing::warn!(run = %id, "run awaits explicit continuation after a lost tool approval");
        continue;
      }
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

/// The runs of `listed`, the `listing` runs, that can be read. One that cannot is never written
/// over or repaired: it is logged with why, and left as it is.
fn readable(listed: Vec<ListedRun<RunStoreError>>, listing: &str) -> Vec<Run> {
  listed
    .into_iter()
    .filter_map(|run| match run {
      Ok(run) => Some(run),
      Err(unreadable) => {
        tracing::warn!(
          run = %unreadable.id,
          listing,
          error = %unreadable.error,
          "a run cannot be read; it is left as it is"
        );
        None
      }
    })
    .collect()
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
    invocation.timing.resume(last_seen);
    db.interrupt_tool_approvals(run.id(), invocation.id)
      .map_err(|error| store(&error))?;
    db.save_invocation(&invocation).map_err(|error| store(&error))?;
  }
  for approval in db.pending_tool_approvals(run.id()).map_err(|error| store(&error))? {
    db.interrupt_tool_approvals(run.id(), approval.invocation)
      .map_err(|error| store(&error))?;
  }
  let events = run.interrupt(cause, Timestamp::now()).map_err(|error| store(&error))?;
  db.append_events(run, &events).map_err(|error| store(&error))
}
