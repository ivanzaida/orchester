//! Returning the work of a run that ended (ORC-44): a person sends a run that succeeded or failed back to one of its
//! agent steps with a note, e.g. to merge its base branch in and resolve the conflicts a merge into it found. The run
//! is reopened and works again in a new attempt from that step: the step runs as a fresh execution whose prompt
//! carries the note ([`Run::returned_note`]), then the steps after it and those that had not passed, as after any
//! return. A run that waits for a person is returned with [`super::StuckDecision::ReturnTo`] instead, and one that
//! never started has no worktree to go on in. Whether the run's work was merged, or its worktree removed, is the
//! store's and the runner's to refuse: the run itself does not know.

use super::{
  attempt::StepExecution,
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  plan::ReviewNote,
  snapshot::StepExecutor,
  state::Run,
  status::{AttemptReason, AttemptStatus, RunFailure, RunStatus, StepStatus},
};
use crate::{
  id::{AttemptId, StepExecutionId},
  resource::StepKey,
  time::Timestamp,
};

impl Run {
  /// Whether the run ended in a way its work can be returned from: it succeeded, or failed after it started.
  pub fn can_return_ended(&self) -> bool {
    match self.status() {
      RunStatus::Succeeded => true,
      RunStatus::Failed(failure) => !matches!(failure, RunFailure::WorktreeNotCreated(_)),
      _ => false,
    }
  }

  /// The agent steps the work of an ended run may return to, in pipeline order: every agent step whose latest
  /// execution passed or failed.
  /// Empty for a run that has not ended so ([`Run::can_return_ended`]).
  pub fn ended_return_targets(&self) -> Vec<StepKey> {
    if !self.can_return_ended() {
      return Vec::new();
    }
    self
      .snapshot()
      .steps()
      .iter()
      .filter(|step| matches!(step.executor(), StepExecutor::Agent { .. }))
      .map(|step| step.key())
      .filter(|key| self.executed(key))
      .cloned()
      .collect()
  }

  /// Whether `step`'s latest execution ran to an end: it passed or failed, not skipped or never started.
  fn executed(&self, step: &StepKey) -> bool {
    self
      .latest_execution(step)
      .is_some_and(|execution| matches!(execution.status(), StepStatus::Passed | StepStatus::Failed))
  }

  /// Reopens the ended run in a new attempt from agent step `step` (one of [`Run::ended_return_targets`]), whose
  /// prompt carries `note` as the person's instruction.
  pub fn return_ended(&mut self, step: &StepKey, note: &ReviewNote, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    if !self.can_return_ended() {
      return Err(RunError::InvalidRunTransition {
        status: self.status().clone(),
        event: RunEventKind::Reopened.name(),
      });
    }
    if !self.ended_return_targets().contains(step) {
      return Err(RunError::NotAReturnTarget(step.clone()));
    }
    self.transition(now, |transition| {
      transition.record(RunEventKind::Reopened)?;
      let executions = transition.run.retry_executions(step);
      transition.record(RunEventKind::AttemptStarted {
        attempt: AttemptId::generate(),
        number: transition.run.next_attempt_number(),
        reason: AttemptReason::ReturnedAfterEnd {
          return_to: step.clone(),
          note: note.clone(),
        },
        executions,
      })
    })
  }

  /// A returned ended run executes an executed agent step and everything after it, in an attempt that comes after
  /// the one the run ended with.
  pub(super) fn check_returned_after_end(
    &self,
    return_to: &StepKey,
    executions: &[PlannedExecution],
  ) -> Result<(), RunError> {
    let ended = self
      .current_attempt()
      .is_some_and(|attempt| matches!(attempt.status(), AttemptStatus::Succeeded | AttemptStatus::Failed));
    if !ended {
      return Err(RunError::InvalidAttempt("work is returned after the run ended"));
    }
    let executed_agent = matches!(
      self.snapshot().step(return_to).map(|step| step.executor()),
      Some(StepExecutor::Agent { .. })
    ) && self.executed(return_to);
    if !executed_agent {
      return Err(RunError::InvalidAttempt("work is returned to an executed agent step"));
    }
    let expected = self.retry_executions(return_to);
    if executions
      .iter()
      .map(|planned| &planned.step)
      .eq(expected.iter().map(|planned| &planned.step))
    {
      Ok(())
    } else {
      Err(RunError::InvalidAttempt(
        "the attempt executes the step it returns to, its dependent and its unfinished steps",
      ))
    }
  }

  /// The person's note `execution`'s prompt carries: it is the execution of the step the work of the ended run was
  /// returned to, in the attempt that returned it. `None` for any other execution.
  pub fn returned_note(&self, execution: StepExecutionId) -> Option<&ReviewNote> {
    let attempt = self
      .attempts()
      .iter()
      .find(|attempt| attempt.execution(execution).is_some())?;
    let AttemptReason::ReturnedAfterEnd { return_to, note } = attempt.reason() else {
      return None;
    };
    attempt
      .execution(execution)
      .map(StepExecution::step)
      .filter(|step| *step == return_to)
      .map(|_| note)
  }
}
