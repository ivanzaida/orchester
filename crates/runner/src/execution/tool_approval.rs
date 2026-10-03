//! Routes one person's answer to the live worker holding that exact tool request. A request no live
//! agent holds can never be answered: the runner ends it as it refuses the answer, so it stops
//! counting as waiting, and the desktop drops it.

use orchester_core::{
  id::{RunId, TaskId},
  run::ApprovalAnswer,
  store::RunStore,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;
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

  /// No task executes `run`, so none of its agents is at work: request `id` ends if it still waits.
  /// The answer is refused either way.
  fn end_unheld(&self, run: RunId, id: &str) -> Rejection {
    let ended = self.open().and_then(|mut db| {
      let ended = db
        .end_waiting_tool_approval(run, id)
        .map_err(|error| failed("cannot end a tool request no agent holds", &error))?;
      if ended {
        announce_ended(&db, &self.inner.notifier, run);
      }
      Ok(())
    });
    match ended {
      Ok(()) => Rejection::ToolApprovalNotAwaited,
      Err(rejection) => rejection,
    }
  }

  /// Ends the requests left waiting by agents that are gone: those of every run this runner does not
  /// execute. Called as a runner attaches, when no agent of an earlier runner can be at work.
  pub(super) fn end_unheld_approvals(&self, db: &mut ProjectDb) -> Result<(), Rejection> {
    let store = |error: &dyn std::error::Error| failed("cannot end the tool requests left waiting", error);
    let mut runs: Vec<RunId> = db
      .pending_tool_approvals_all()
      .map_err(|error| store(&error))?
      .into_iter()
      .map(|approval| approval.run)
      .filter(|run| !self.is_active(*run))
      .collect();
    runs.sort_unstable();
    runs.dedup();
    for run in runs {
      let ended = db.end_waiting_tool_approvals(run).map_err(|error| store(&error))?;
      if ended > 0 {
        tracing::info!(%run, ended, "tool requests left waiting by a gone agent ended");
        announce_ended(db, &self.inner.notifier, run);
      }
    }
    Ok(())
  }
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
      answer(ack, Err(Rejection::ToolApprovalNotAwaited));
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
    if self.db.end_waiting_tool_approval(self.run.id(), id)? {
      notify(&self.notifier, self.run.id(), self.task.id());
    }
    answer(ack, Err(Rejection::ToolApprovalNotAwaited));
    Ok(())
  }

  /// The run's task no longer executes it and none of its agents is at work: whatever request is still
  /// recorded as waiting can never be answered, and ends.
  pub(super) fn end_waiting_approvals(&mut self) {
    match self.db.end_waiting_tool_approvals(self.run.id()) {
      Ok(0) => {}
      Ok(ended) => {
        tracing::info!(run = %self.run.id(), ended, "tool requests of agents that ended no longer wait");
        notify(&self.notifier, self.run.id(), self.task.id());
      }
      // The runner keeps going; the next runner to attach ends them.
      Err(error) => tracing::error!(run = %self.run.id(), %error, "cannot end the tool requests left waiting"),
    }
  }
}
