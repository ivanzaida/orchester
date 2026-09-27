//! Follow-ups (Ask agent, 02.13): a person asks about a run that succeeded. The run works again in
//! an attempt of its own with one execution of the step asked, which continues the CLI session of
//! that step's passed execution in the run's worktree and makes new checkpoints; the run is ready for
//! review again once it passes. The run's limits apply as to any work. A follow-up that fails leaves
//! the run as it was: succeeded, with the result it had; its attempt stays recorded as failed.

use super::{
  attempt::StepExecution,
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  snapshot::StepExecutor,
  state::Run,
  status::{AttemptReason, AttemptStatus, RunStatus, StepStatus},
};
use crate::{
  id::{AttemptId, ResourceId, StepExecutionId},
  resource::StepKey,
  time::Timestamp,
};

impl Run {
  /// The step a follow-up asks: the last agent step in pipeline order — of agent profile `profile`,
  /// when given — whose latest execution passed. `None` when the run has none.
  pub fn follow_up_step(&self, profile: Option<ResourceId>) -> Option<StepKey> {
    self
      .snapshot()
      .steps()
      .iter()
      .rev()
      .filter(|step| match step.executor() {
        StepExecutor::Agent { profile: pinned, .. } => profile.is_none_or(|profile| pinned.resource == profile),
        StepExecutor::Tool { .. } => false,
      })
      .map(|step| step.key())
      .find(|key| self.latest_execution(key).map(StepExecution::status) == Some(StepStatus::Passed))
      .cloned()
  }

  /// Reopens a succeeded run for a follow-up of `step`, an agent step whose latest execution passed:
  /// a new attempt with one execution of it, which [`Run::followed_up`] names the execution it
  /// continues.
  pub fn follow_up(&mut self, step: &StepKey, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::Reopened)?;
      transition.record(RunEventKind::AttemptStarted {
        attempt: AttemptId::generate(),
        number: transition.run.next_attempt_number(),
        reason: AttemptReason::FollowUp { step: step.clone() },
        executions: vec![PlannedExecution {
          id: StepExecutionId::generate(),
          step: step.clone(),
        }],
      })
    })
  }

  /// Whether `execution` belongs to a follow-up attempt.
  pub fn is_follow_up(&self, execution: StepExecutionId) -> bool {
    self
      .attempts()
      .iter()
      .find(|attempt| attempt.execution(execution).is_some())
      .is_some_and(|attempt| matches!(attempt.reason(), AttemptReason::FollowUp { .. }))
  }

  /// The passed execution a follow-up's `execution` continues: its step's latest one before the
  /// follow-up, whose CLI session it resumes. `None` for any other execution.
  pub fn followed_up(&self, execution: StepExecutionId) -> Option<StepExecutionId> {
    let position = self
      .attempts()
      .iter()
      .position(|attempt| attempt.execution(execution).is_some())?;
    let AttemptReason::FollowUp { step } = self.attempts()[position].reason() else {
      return None;
    };
    self.attempts()[..position]
      .iter()
      .rev()
      .filter(|attempt| attempt.counts())
      .find_map(|attempt| attempt.executions().iter().find(|earlier| earlier.step() == step))
      .map(StepExecution::id)
  }

  /// Whether the run works on a follow-up: its current attempt is one and still active.
  pub(super) fn in_follow_up(&self) -> bool {
    self.status() == &RunStatus::Running
      && self.current_attempt().is_some_and(|attempt| {
        attempt.status() == AttemptStatus::Active && matches!(attempt.reason(), AttemptReason::FollowUp { .. })
      })
  }

  /// A follow-up attempt executes exactly the agent step it asks, of a run whose steps all passed.
  pub(super) fn check_follow_up(&self, step: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    if !matches!(executions, [only] if only.step == *step) {
      return Err(RunError::InvalidAttempt("a follow-up executes the one step it asks"));
    }
    if !matches!(
      self.snapshot().step(step).map(|pinned| pinned.executor()),
      Some(StepExecutor::Agent { .. })
    ) {
      return Err(RunError::InvalidAttempt("a follow-up asks an agent step"));
    }
    if !self.all_passed() {
      return Err(RunError::InvalidAttempt(
        "only a run whose steps all passed is followed up",
      ));
    }
    Ok(())
  }
}
