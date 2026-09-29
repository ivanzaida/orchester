//! A message can resume a failed run without discarding its executions, branch or sessions.

use super::{
  AttemptReason, AttemptStatus, PlannedExecution, Run, RunError, RunEvent, RunEventKind, RunFailure, RunStatus,
  StepExecutor, StepStatus,
};
use crate::{
  id::{AttemptId, ResourceId, StepExecutionId},
  resource::StepKey,
  time::Timestamp,
};

impl Run {
  /// The agent a message can resume after failure. Orchestrator selects the failed agent, or
  /// the latest executed upstream agent when a command step failed. An explicit profile may
  /// instead revisit its own executed step; its downstream results are then invalidated.
  pub fn recovery_step(&self, profile: Option<ResourceId>) -> Option<StepKey> {
    let RunStatus::Failed(RunFailure::StepFailed(failed) | RunFailure::RetriesExhausted(failed)) = self.status() else {
      return None;
    };
    self.snapshot().steps().iter().rev().find_map(|step| {
      let StepExecutor::Agent { profile: pinned, .. } = step.executor() else {
        return None;
      };
      let reaches = match profile {
        Some(profile) => profile == pinned.resource,
        None => step.key() == failed || self.snapshot().downstream_of(step.key()).contains(failed),
      };
      (reaches && self.recoverable_step(step.key())).then(|| step.key().clone())
    })
  }

  fn recoverable_step(&self, step: &StepKey) -> bool {
    matches!(
      self.snapshot().step(step).map(|step| step.executor()),
      Some(StepExecutor::Agent { .. })
    ) && self
      .latest_execution(step)
      .is_some_and(|execution| matches!(execution.status(), StepStatus::Passed | StepStatus::Failed))
  }

  /// Resumes the selected agent in a new attempt of the same failed run. Completed upstream
  /// steps remain passed; selected, dependent and unfinished steps execute again.
  pub fn recover(&mut self, step: &StepKey, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::RecoveryRequested)?;
      transition.record(RunEventKind::AttemptStarted {
        attempt: AttemptId::generate(),
        number: transition.run.next_attempt_number(),
        reason: AttemptReason::Recovery { step: step.clone() },
        executions: transition.run.retry_executions(step),
      })
    })
  }

  pub(super) fn check_recovery(&self, step: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    if !self
      .current_attempt()
      .is_some_and(|attempt| attempt.status() == AttemptStatus::Failed)
      || !self.recoverable_step(step)
    {
      return Err(RunError::InvalidAttempt(
        "recovery resumes an executed agent of a failed attempt",
      ));
    }
    let expected = self.retry_executions(step);
    if executions
      .iter()
      .map(|item| &item.step)
      .eq(expected.iter().map(|item| &item.step))
    {
      Ok(())
    } else {
      Err(RunError::InvalidAttempt(
        "recovery executes the selected, dependent and unfinished steps",
      ))
    }
  }

  /// The execution whose session and surviving worktree this recovery execution continues.
  /// Passed downstream steps execute afresh; unfinished parallel steps retain their partial work.
  pub fn recovered_execution(&self, execution: StepExecutionId) -> Option<StepExecutionId> {
    let position = self
      .attempts()
      .iter()
      .position(|attempt| attempt.execution(execution).is_some())?;
    let attempt = &self.attempts()[position];
    let AttemptReason::Recovery { step } = attempt.reason() else {
      return None;
    };
    let current = attempt.execution(execution)?;
    let earlier = self.previous_execution(execution).and_then(|id| self.execution(id))?;
    (current.step() == step || earlier.status() != StepStatus::Passed).then(|| earlier.id())
  }

  /// The previous execution of the same step, including attempts whose pending execution never
  /// started. The runner walks this history to find a real session or surviving worktree.
  pub fn previous_execution(&self, execution: StepExecutionId) -> Option<StepExecutionId> {
    let position = self
      .attempts()
      .iter()
      .position(|attempt| attempt.execution(execution).is_some())?;
    let current = self.attempts()[position].execution(execution)?;
    self.attempts()[..position]
      .iter()
      .rev()
      .filter(|attempt| attempt.counts())
      .find_map(|attempt| {
        attempt
          .executions()
          .iter()
          .find(|earlier| earlier.step() == current.step())
      })
      .map(|earlier| earlier.id())
  }
}
