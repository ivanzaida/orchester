use std::num::NonZeroU32;

use super::status::{AttemptReason, AttemptStatus, StepFailure, StepStatus};
use crate::{
  id::{AttemptId, CheckpointId, StepExecutionId},
  resource::StepKey,
  revision::Revision,
  time::Timestamp,
};

/// One execution of a pipeline step within an attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepExecution {
  id: StepExecutionId,
  step: StepKey,
  status: StepStatus,
  failure: Option<StepFailure>,
  /// The revision of the run's branch a failed execution's result was for, when it is known.
  judged: Option<Revision>,
}

impl StepExecution {
  /// Rebuilds a persisted execution; [`super::Run::restore`] checks it against the run.
  pub const fn new(id: StepExecutionId, step: StepKey, status: StepStatus) -> Self {
    Self {
      id,
      step,
      status,
      failure: None,
      judged: None,
    }
  }

  /// A persisted failed execution with the reason it failed for.
  #[must_use]
  pub const fn with_failure(mut self, failure: Option<StepFailure>) -> Self {
    self.failure = failure;
    self
  }

  /// A persisted failed execution with the revision its result was for.
  #[must_use]
  pub fn with_judged(mut self, judged: Option<Revision>) -> Self {
    self.judged = judged;
    self
  }

  pub const fn id(&self) -> StepExecutionId {
    self.id
  }

  pub const fn step(&self) -> &StepKey {
    &self.step
  }

  pub const fn status(&self) -> StepStatus {
    self.status
  }

  /// Why a failed execution failed, when its log says.
  pub const fn failure(&self) -> Option<StepFailure> {
    self.failure
  }

  /// The revision of the run's branch a failed execution's result was for, when it is known.
  pub const fn judged(&self) -> Option<&Revision> {
    self.judged.as_ref()
  }

  pub(super) const fn set_status(&mut self, status: StepStatus) {
    self.status = status;
  }

  pub(super) fn fail(&mut self, failure: Option<StepFailure>, judged: Option<Revision>) {
    self.status = StepStatus::Failed;
    self.failure = failure;
    self.judged = judged;
  }

  /// A person accepted it as it is: it passed, with no failure left.
  pub(super) fn accept(&mut self) {
    self.status = StepStatus::Passed;
    self.failure = None;
    self.judged = None;
  }
}

/// One pass through the pipeline. Attempt 1 executes every step; a retry attempt executes
/// only the steps its failure route reset, the others keep their earlier result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
  id: AttemptId,
  number: NonZeroU32,
  reason: AttemptReason,
  status: AttemptStatus,
  executions: Vec<StepExecution>,
}

impl Attempt {
  /// Rebuilds a persisted attempt; [`super::Run::restore`] checks it against the run.
  pub const fn new(
    id: AttemptId,
    number: NonZeroU32,
    reason: AttemptReason,
    status: AttemptStatus,
    executions: Vec<StepExecution>,
  ) -> Self {
    Self {
      id,
      number,
      reason,
      status,
      executions,
    }
  }

  pub const fn id(&self) -> AttemptId {
    self.id
  }

  pub const fn number(&self) -> NonZeroU32 {
    self.number
  }

  pub const fn reason(&self) -> &AttemptReason {
    &self.reason
  }

  pub const fn status(&self) -> AttemptStatus {
    self.status
  }

  pub fn executions(&self) -> &[StepExecution] {
    &self.executions
  }

  /// Whether its executions decide their steps' results: every attempt but a follow-up that failed,
  /// which left the run with the result it had.
  pub fn counts(&self) -> bool {
    !(self.status == AttemptStatus::Failed && matches!(self.reason, AttemptReason::FollowUp { .. }))
  }

  pub fn execution(&self, id: StepExecutionId) -> Option<&StepExecution> {
    self.executions.iter().find(|execution| execution.id == id)
  }

  pub(super) fn execution_mut(&mut self, id: StepExecutionId) -> Option<&mut StepExecution> {
    self.executions.iter_mut().find(|execution| execution.id == id)
  }

  pub(super) const fn set_status(&mut self, status: AttemptStatus) {
    self.status = status;
  }
}

/// A persisted point in a step execution a run can continue from: the committed revision of
/// the run's worktree at that moment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
  id: CheckpointId,
  execution: StepExecutionId,
  revision: Revision,
  created_at: Timestamp,
}

impl Checkpoint {
  pub const fn new(id: CheckpointId, execution: StepExecutionId, revision: Revision, created_at: Timestamp) -> Self {
    Self {
      id,
      execution,
      revision,
      created_at,
    }
  }

  pub const fn id(&self) -> CheckpointId {
    self.id
  }

  pub const fn execution(&self) -> StepExecutionId {
    self.execution
  }

  pub const fn revision(&self) -> &Revision {
    &self.revision
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }
}
