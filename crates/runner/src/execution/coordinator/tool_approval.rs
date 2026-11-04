//! A person's answer to a tool request (`crate::execution::executor::tool_approval`), as the scheduler takes it: it
//! goes to the task executing the run; while none does, no agent can hold the request, which ends as the answer
//! is refused.

use orchester_core::{
  id::RunId,
  run::{Answerer, ApprovalAnswer, PendingToolApproval},
  store::RunStore,
};
use orchester_ipc::Rejection;
use orchester_store::{ApprovalRecord, ProjectDb};

use super::{Notifier, Scheduler, failed, link::LocalLink, sink::LocalSink};
use crate::execution::executor::{
  control::Control,
  sink::Sink,
  tool_approval::{notify, unheld_refusal},
};

impl Scheduler {
  pub async fn decide_tool_approval(
    &self,
    run: RunId,
    approval: String,
    answer: ApprovalAnswer,
    by: Answerer,
  ) -> Result<(), Rejection> {
    let id = approval.clone();
    let Some(reply) = self.ask(run, |ack| Control::DecideToolApproval {
      approval,
      answer,
      by,
      ack,
    }) else {
      return Err(self.end_unheld(run, &id));
    };
    // A task that drops the request has no agent left that could hold it.
    reply.await.unwrap_or_else(|_| Err(self.end_unheld(run, &id)))
  }

  /// No task executes `run`, so none of its agents is at work: request `id` ends if it still waits,
  /// and the answer is refused.
  fn end_unheld(&self, run: RunId, id: &str) -> Rejection {
    let failure = |error: &dyn std::error::Error| failed("cannot end a tool request no agent holds", error);
    let refused = self.open().and_then(|db| {
      let mut sink = LocalSink::new(&self.inner.project_root);
      let ended = sink
        .approval(ApprovalRecord::ended(run, id))
        .map_err(|error| failure(&error))?;
      if ended {
        announce_ended(&db, &self.inner.notifier, run);
      }
      unheld_refusal(ended, &LocalLink::over(&db), run, id).map_err(|error| failure(&error))
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

/// Tells the desktop that requests of `run` ended. A run that cannot be read names no task to tell
/// about; the desktop reads every request again once the runner attached or answered anyway.
fn announce_ended(db: &ProjectDb, notifier: &Notifier, run: RunId) {
  match db.run(run) {
    Ok(Some(found)) => notify(notifier, run, found.task()),
    Ok(None) => tracing::warn!(%run, "an ended tool request names no run"),
    Err(error) => tracing::warn!(%run, %error, "the run of an ended tool request cannot be read"),
  }
}
