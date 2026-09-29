//! Routes one person's decision to the live worker holding that exact Codex JSON-RPC request.

use orchester_ipc::Rejection;

use super::{
  Scheduler,
  control::{Ack, Control, answer},
  run::{ExecutionError, RunTask},
  step::worker::ApprovalDecision,
};
use orchester_core::id::RunId;

impl Scheduler {
  pub async fn decide_tool_approval(&self, run: RunId, approval: String, approve: bool) -> Result<(), Rejection> {
    let Some(reply) = self.ask(run, |ack| Control::DecideToolApproval { approval, approve, ack }) else {
      return Err(Rejection::ToolApprovalNotAwaited);
    };
    reply.await.unwrap_or(Err(Rejection::ToolApprovalNotAwaited))
  }
}

impl RunTask {
  pub(super) fn decide_tool_approval(&mut self, id: String, approve: bool, ack: Ack) -> Result<(), ExecutionError> {
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
      answer(ack, Err(Rejection::ToolApprovalNotAwaited));
      return Ok(());
    };
    if worker.send(ApprovalDecision { id, approve, ack }).is_err() {
      // The worker has ended; its finalization marks the row interrupted.
      tracing::debug!(run = %self.run.id(), "tool decision arrived after the Codex worker ended");
    }
    Ok(())
  }
}
