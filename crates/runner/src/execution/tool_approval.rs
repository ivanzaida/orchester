//! Routes one person's answer to the live worker holding that exact tool request. A request no live
//! agent holds can never be answered: the runner ends it as it refuses the answer, so it stops
//! counting as waiting, and the desktop drops it. An answer to a request answered already is refused
//! as such, so the desktop does not say it ended.

use orchester_core::{
  id::{RunId, TaskId},
  run::{ApprovalAnswer, PendingToolApproval},
  store::RunStore,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ProjectDb, StoreError};
use tokio::sync::mpsc::error::SendError;

use super::{
  Notifier, Scheduler,
  control::{Ack, Control, answer},
  failed,
  run::{ExecutionError, RunTask},
  step::worker::ApprovalDecision,
};

impl Scheduler {
  pub async fn decide_tool_approval(
    &self,
    run: RunId,
    approval: String,
    answer: ApprovalAnswer,
  ) -> Result<(), Rejection> {
    let id = approval.clone();
    let Some(reply) = self.ask(run, |ack| Control::DecideToolApproval { approval, answer, ack }) else {
      return Err(self.end_unheld(run, &id));
    };
    // A task that drops the request has no agent left that could hold it.
    reply.await.unwrap_or_else(|_| Err(self.end_unheld(run, &id)))
  }

  /// No task executes `run`, so none of its agents is at work: request `id` ends if it still waits,
  /// and the answer is refused.
  fn end_unheld(&self, run: RunId, id: &str) -> Rejection {
    let refused = self.open().and_then(|mut db| {
      let (ended, refusal) =
        end_unheld(&mut db, run, id).map_err(|error| failed("cannot end a tool request no agent holds", &error))?;
      if ended {
        announce_ended(&db, &self.inner.notifier, run);
      }
      Ok(refusal)
    });
    refused.unwrap_or_else(|rejection| rejection)
  }

  /// Ends the requests left waiting by agents that are gone: those read as waiting in every run this
  /// runner does not execute. Called as a runner attaches, when no agent of an earlier runner can be at
  /// work; a request recorded after they were read is left to its own agent.
  pub(super) fn end_unheld_approvals(&self, db: &mut ProjectDb) -> Result<(), Rejection> {
    let store = |error: &dyn std::error::Error| failed("cannot end the tool requests left waiting", error);
    let unheld: Vec<PendingToolApproval> = db
      .pending_tool_approvals_all()
      .map_err(|error| store(&error))?
      .into_iter()
      .filter(|approval| !self.is_active(approval.run))
      .collect();
    let mut runs = Vec::new();
    for approval in unheld {
      if db
        .end_waiting_tool_approval(approval.run, &approval.id)
        .map_err(|error| store(&error))?
      {
        runs.push(approval.run);
      }
    }
    runs.sort_unstable();
    runs.dedup();
    for run in runs {
      tracing::info!(%run, "tool requests left waiting by a gone agent ended");
      announce_ended(db, &self.inner.notifier, run);
    }
    Ok(())
  }
}

/// Ends request `id` of `run`, which no live agent holds, if it still waits: whether it did, and why its
/// answer is refused ([`refusal`]).
pub(super) fn end_unheld(db: &mut ProjectDb, run: RunId, id: &str) -> Result<(bool, Rejection), StoreError> {
  if db.end_waiting_tool_approval(run, id)? {
    return Ok((true, Rejection::ToolApprovalNotAwaited));
  }
  Ok((false, refusal(db, run, id)?))
}

/// Why an answer to request `id` of `run`, which no longer waits, is refused: it was answered already,
/// or it ended without an answer.
pub(super) fn refusal(db: &ProjectDb, run: RunId, id: &str) -> Result<Rejection, StoreError> {
  Ok(if db.tool_approval_answered(run, id)? {
    Rejection::ToolApprovalAnswered
  } else {
    Rejection::ToolApprovalNotAwaited
  })
}

/// Tells the desktop that requests of `run` ended. A run that cannot be read names no task to tell
/// about; the desktop reads every request again once the runner attached or answered anyway.
fn announce_ended(db: &ProjectDb, notifier: &Notifier, run: RunId) {
  match db.run(run) {
    Ok(Some(found)) => notify(notifier, run, found.task()),
    Ok(None) => tracing::warn!(%run, "an ended tool request names no run"),
    Err(error) => tracing::warn!(%run, %error, "the run of an ended tool request cannot be read"),
  }
}

fn notify(notifier: &Notifier, run: RunId, task: TaskId) {
  notifier.send(Event::ToolApprovalChanged { run, task });
}

impl RunTask {
  pub(super) fn decide_tool_approval(
    &mut self,
    id: String,
    given: ApprovalAnswer,
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
    if let Err(SendError(decision)) = worker.send(ApprovalDecision { id, answer: given, ack }) {
      // The worker has ended, and with it the call it waited on: whatever of it still waits can no longer
      // be answered.
      tracing::debug!(run = %self.run.id(), "tool decision arrived after the agent's worker ended");
      return self.end_unheld(&decision.id, decision.ack);
    }
    Ok(())
  }

  /// No agent of the run holds request `id`: it ends if it still waits, and the answer is refused.
  fn end_unheld(&mut self, id: &str, ack: Ack) -> Result<(), ExecutionError> {
    let (ended, refusal) = end_unheld(&mut self.db, self.run.id(), id)?;
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
        Ok(self.db.end_waiting_tool_approval(run, &approval.id)? || ended)
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
