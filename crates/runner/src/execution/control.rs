//! Requests the scheduler hands to the task that executes a run. The task owns the run's state,
//! so every change to an executing run goes through it.

use orchester_core::{
  id::ResourceId,
  question::GivenAnswer,
  run::{Answerer, ApprovalAnswer, InterruptCause},
};
use orchester_ipc::Rejection;
use tokio::sync::oneshot;

use super::{permissions::PermissionsChange, plan::PlanRequest};

/// Answers a request once it is carried out.
pub(super) type Ack = oneshot::Sender<Result<(), Rejection>>;

pub(super) enum Control {
  /// Pause at the next checkpoint; answered once the request is recorded.
  Pause(Ack),
  /// Cancel now; answered once the run is recorded as cancelled.
  Cancel(Ack),
  /// Change a profile's permissions for the run; answered once the change is recorded.
  ChangePermissions(PermissionsChange, Ack),
  /// Keep a pending change of a profile's permissions for the next invocations.
  KeepPermissions(ResourceId, Ack),
  /// Record a person's decision on a plan; answered once it is recorded.
  DecidePlan(PlanRequest, Ack),
  /// Send a person's answer to the worker that owns the pending tool request.
  DecideToolApproval {
    approval: String,
    answer: ApprovalAnswer,
    by: Answerer,
    ack: Ack,
  },
  /// Send answers to the worker whose agent waits on the question request in its turn.
  AnswerQuestion {
    request: String,
    answers: Vec<GivenAnswer>,
    by: Answerer,
    ack: Ack,
  },
  /// People's messages were stored; answered at once, delivered at the next boundary.
  DeliverMessages(Ack),
  /// The runner stops: end the agent and record the run as interrupted.
  Interrupt(InterruptCause),
}

/// Answers `ack`; the requester may have stopped waiting, and then the answer is not needed.
pub(super) fn answer(ack: Ack, result: Result<(), Rejection>) {
  if ack.send(result).is_err() {
    tracing::debug!("nobody waits for the answer anymore");
  }
}
