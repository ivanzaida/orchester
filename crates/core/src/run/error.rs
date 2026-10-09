use thiserror::Error;

use super::{
  plan::PlanVersion,
  status::{InterruptCause, RunLimit, RunStatus, StepStatus},
};
use crate::{
  id::{CheckpointId, ResourceId, RunId, StepExecutionId},
  permission::capability::PermissionProblem,
  resource::StepKey,
};

/// A rejected run transition or an event log that does not describe a valid run.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RunError {
  #[error("a {status:?} run cannot accept `{event}`")]
  InvalidRunTransition { status: RunStatus, event: &'static str },
  #[error("step `{step}` is {status:?} and cannot accept `{event}`")]
  InvalidStepTransition {
    step: StepKey,
    status: StepStatus,
    event: &'static str,
  },
  #[error("step execution {0} does not belong to this run")]
  UnknownExecution(StepExecutionId),
  #[error("step execution {0} belongs to an earlier attempt")]
  NotInCurrentAttempt(StepExecutionId),
  #[error("step `{0}` cannot start before every step it depends on has passed")]
  DependenciesNotPassed(StepKey),
  #[error("step `{0}` has an approval gate; its result passes only through a plan decision")]
  GatedStep(StepKey),
  #[error("step `{0}` has no approval gate, so its result needs no decision")]
  NotGated(StepKey),
  #[error("plan v{0} has been decided already; a revised plan needs a decision of its own")]
  PlanAlreadyDecided(PlanVersion),
  #[error("the current attempt still has open step executions")]
  OpenExecutions,
  #[error("the run is not working on a follow-up")]
  NotFollowingUp,
  #[error("the run cannot succeed before every step has passed")]
  StepsNotPassed,
  #[error("the attempt does not fit the run: {0}")]
  InvalidAttempt(&'static str),
  #[error("a run continues from its last checkpoint {expected:?}, not {found:?}")]
  CheckpointMismatch {
    expected: Option<CheckpointId>,
    found: Option<CheckpointId>,
  },
  #[error("the run was interrupted by {0:?} and continues only when a person asks")]
  NeedsExplicitContinue(InterruptCause),
  #[error("the time limit must be greater than zero")]
  ZeroDuration,
  #[error("the run stopped at its {0:?} limit and continues only with a raised limit")]
  LimitNotRaised(RunLimit),
  #[error("the new {0:?} limit must be above what the run already used")]
  LimitNotAboveUsed(RunLimit),

  #[error("no step of the run uses agent profile {0}")]
  ProfileNotInRun(ResourceId),
  #[error("step `{step}` cannot run with these permissions: {problem}")]
  PermissionsNotEnforceable { step: StepKey, problem: PermissionProblem },
  #[error("no agent of profile {0} is at work, so none can restart with new permissions now")]
  NoAgentToRestart(ResourceId),
  #[error("no change of profile {0}'s permissions waits for a checkpoint")]
  NoPendingPermissions(ResourceId),
  #[error("step execution {0} is not waiting to restart with new permissions")]
  NotRestartingForPermissions(StepExecutionId),
  #[error("a run log starts with exactly one creation event at sequence 1")]
  Creation,
  #[error("event of run {found} appears in the log of run {expected}")]
  WrongRun { expected: RunId, found: RunId },
  #[error("event {found} does not follow event {previous}")]
  OutOfSequence { previous: u64, found: u64 },
  #[error("step `{0}` has no failure route to retry")]
  NoFailureRoute(StepKey),
  #[error("the work cannot return to step `{0}`: it is no agent step the failed step runs after")]
  NotAReturnTarget(StepKey),
  #[error("the stored run is inconsistent: {0}")]
  Inconsistent(&'static str),
}
