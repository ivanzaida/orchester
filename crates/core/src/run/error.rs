use thiserror::Error;

use super::status::{InterruptCause, RunStatus, StepStatus};
use crate::{
  id::{CheckpointId, RunId, StepExecutionId},
  money::Currency,
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
  #[error("the current attempt still has open step executions")]
  OpenExecutions,
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
  #[error("the budget must be greater than zero")]
  NonPositiveBudget,
  #[error("spending in {spent:?} does not match the budget currency {budget:?}")]
  CurrencyMismatch { budget: Currency, spent: Currency },
  #[error("a recorded spend cannot be negative")]
  NegativeSpend,
  #[error("the total spent no longer fits in micro-units")]
  SpendOverflow,
  #[error("a run log starts with exactly one creation event at sequence 1")]
  Creation,
  #[error("event of run {found} appears in the log of run {expected}")]
  WrongRun { expected: RunId, found: RunId },
  #[error("event {found} does not follow event {previous}")]
  OutOfSequence { previous: u64, found: u64 },
  #[error("the stored run is inconsistent: {0}")]
  Inconsistent(&'static str),
}
