//! Routes one person's answer to the live worker holding that exact tool request; the scheduler's side is
//! `crate::execution::coordinator::tool_approval`'s. A request no live agent holds can never be answered: the
//! runner ends it as it refuses the answer, so it stops counting as waiting, and the desktop drops it. An answer
//! to a request answered already is refused as such, so the desktop does not say it ended.

use orchester_core::{
  id::{RunId, TaskId},
  run::{Answerer, ApprovalAnswer},
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ApprovalRecord, ProjectDb, StoreError};
use tokio::sync::mpsc::error::SendError;

use super::{
  Notifier,
  control::{Ack, answer},
  run::{ExecutionError, RunTask},
  sink::Sink,
  step::worker::ApprovalDecision,
};

/// Ends request `id` of `run`, which no live agent holds, through `sink` if it still waits: whether it did, and why
/// its answer is refused ([`refusal`]), read in `db`.
pub(in crate::execution) fn end_unheld(
  (db, sink): (&ProjectDb, &mut dyn Sink),
  run: RunId,
  id: &str,
) -> Result<(bool, Rejection), StoreError> {
  if sink.approval(ApprovalRecord::ended(run, id))? {
    return Ok((true, Rejection::ToolApprovalNotAwaited));
  }
  Ok((false, refusal(db, run, id)?))
}

/// Why an answer to request `id` of `run`, which no longer waits, is refused: it was answered already,
/// or it ended without an answer.
pub(in crate::execution) fn refusal(db: &ProjectDb, run: RunId, id: &str) -> Result<Rejection, StoreError> {
  Ok(if db.tool_approval_answered(run, id)? {
    Rejection::ToolApprovalAnswered
  } else {
    Rejection::ToolApprovalNotAwaited
  })
}

pub(in crate::execution) fn notify(notifier: &Notifier, run: RunId, task: TaskId) {
  notifier.send(Event::ToolApprovalChanged { run, task });
}

impl RunTask {
  pub(super) fn decide_tool_approval(
    &mut self,
    id: String,
    (given, by): (ApprovalAnswer, Answerer),
    ack: Ack,
  ) -> Result<(), ExecutionError> {
    let pending = self.db.pending_tool_approvals(self.run.id())?;
    let Some(approval) = pending.iter().find(|approval| approval.id == id) else {
      answer(ack, Err(refusal(&self.db, self.run.id(), &id)?));
      return Ok(());
    };
    let worker = self
      .agents
      .get(&approval.execution)
      .and_then(|active| active.approvals.as_ref());
    let Some(worker) = worker else {
      return self.end_unheld(&id, ack);
    };
    if let Err(SendError(decision)) = worker.send(ApprovalDecision {
      id,
      answer: given,
      by,
      ack,
    }) {
      // The worker has ended, and with it the call it waited on: whatever of it still waits can no longer
      // be answered.
      tracing::debug!(run = %self.run.id(), "tool decision arrived after the agent's worker ended");
      return self.end_unheld(&decision.id, decision.ack);
    }
    Ok(())
  }

  /// No agent of the run holds request `id`: it ends if it still waits, and the answer is refused.
  fn end_unheld(&mut self, id: &str, ack: Ack) -> Result<(), ExecutionError> {
    let (ended, refusal) = end_unheld((&self.db, self.sink.as_mut()), self.run.id(), id)?;
    if ended {
      notify(&self.notifier, self.run.id(), self.task.id());
    }
    answer(ack, Err(refusal));
    Ok(())
  }

  /// The run's task no longer executes it and none of its agents is at work: the requests read as still
  /// waiting can never be answered, and end.
  pub(super) fn end_waiting_approvals(&mut self) {
    let run = self.run.id();
    let ended = self.db.pending_tool_approvals(run).and_then(|waiting| {
      waiting.iter().try_fold(false, |ended, approval| {
        Ok(self.sink.approval(ApprovalRecord::ended(run, &approval.id))? || ended)
      })
    });
    match ended {
      Ok(false) => {}
      Ok(true) => {
        tracing::info!(%run, "tool requests of agents that ended no longer wait");
        notify(&self.notifier, run, self.task.id());
      }
      // The runner keeps going; the next runner to attach ends them.
      Err(error) => tracing::error!(%run, %error, "cannot end the tool requests left waiting"),
    }
  }
}
