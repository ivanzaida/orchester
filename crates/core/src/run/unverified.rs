//! A step that waits for a person without a verdict on the work (`.tmp/analysis/agent-dev-blockers.md` §6): its
//! agent reported `unverified`, as it could not check everything the step is for, or the step was not run again on
//! the revision its latest failed or unverified result was for. Neither takes the step's failure route nor uses a
//! retry. The person accepts the step as it is, returns the work to an agent step with a note, or runs the step
//! again ([`super::StuckDecision`]).
//!
//! A step whose latest result holds for its revision ([`StepFailure::holds_for_revision`]) is not run again on that
//! revision unless a person asked for it: they restarted that step, returned the work to it, or recovered the run.
//! The seed scenarios and fixtures a verification uses are files of the repository, so a change to them is a new
//! revision as well.

use super::{
  attempt::StepExecution,
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  state::Run,
  status::{AttemptReason, StepFailure, StepStatus, Stuck},
};
use crate::{
  id::{AttemptId, StepExecutionId},
  resource::StepKey,
  revision::Revision,
  time::Timestamp,
};

impl Run {
  /// Whether starting `execution`, pending in the current attempt, would run its step again on `revision` of the
  /// run's branch, which the step's latest result was failed or unverified for, without a person asking for it.
  pub fn reruns_unchanged(&self, execution: StepExecutionId, revision: &Revision) -> bool {
    let Some((attempt, earlier)) = self.attempts().split_last() else {
      return false;
    };
    let Some(pending) = attempt
      .execution(execution)
      .filter(|pending| pending.status() == StepStatus::Pending)
    else {
      return false;
    };
    let step = pending.step();
    let asked = match attempt.reason() {
      AttemptReason::Retry { .. } | AttemptReason::Accepted { .. } => false,
      AttemptReason::Returned { return_to, .. } | AttemptReason::Restart { step: return_to } => return_to == step,
      AttemptReason::Initial | AttemptReason::FollowUp { .. } | AttemptReason::Recovery { .. } => true,
    };
    if asked {
      return false;
    }
    earlier
      .iter()
      .rev()
      .filter(|attempt| attempt.counts())
      .flat_map(|attempt| attempt.executions())
      .find(|judged| judged.step() == step && matches!(judged.status(), StepStatus::Passed | StepStatus::Failed))
      .is_some_and(|judged| {
        judged.failure().is_some_and(StepFailure::holds_for_revision) && judged.judged() == Some(revision)
      })
  }

  /// Does not run `execution` ([`Run::reruns_unchanged`]): it fails unchanged on `revision`, the attempt ends and
  /// the run waits for a person.
  pub fn refuse_unchanged(
    &mut self,
    execution: StepExecutionId,
    revision: Revision,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::StepStarted { execution })?;
      transition.record(RunEventKind::StepFailed {
        execution,
        reason: Some(StepFailure::Unchanged),
        revision: Some(revision),
      })?;
      transition.skip_open()?;
      let step = transition
        .run
        .execution(execution)
        .expect("StepFailed was accepted for this execution")
        .step()
        .clone();
      transition.record(RunEventKind::WaitingForPerson {
        reason: Stuck::Unchanged(step),
      })
    })
  }

  /// [`super::StuckDecision::Accept`] of a run waiting for `stuck`: the step's latest execution passes, and the run
  /// succeeds or runs the steps that have not passed.
  pub(super) fn accept_stuck(&mut self, stuck: &Stuck, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    let step = stuck.step().clone();
    if !stuck.can_accept() {
      return Err(RunError::NotAcceptable(step));
    }
    let execution = self
      .latest_execution(&step)
      .map(StepExecution::id)
      .ok_or_else(|| RunError::NotAcceptable(step.clone()))?;
    self.transition(now, |transition| {
      transition.record(RunEventKind::RetryAllowed)?;
      transition.record(RunEventKind::StepAccepted { execution })?;
      if transition.run.all_passed() {
        return transition.record(RunEventKind::Succeeded);
      }
      let attempt = RunEventKind::AttemptStarted {
        attempt: AttemptId::generate(),
        number: transition.run.next_attempt_number(),
        executions: transition.run.unpassed_executions(),
        reason: AttemptReason::Accepted { step },
      };
      transition.record(attempt)
    })
  }

  /// A person's acceptance passes the latest execution of its step, which failed unverified or unchanged, once the
  /// attempt it ended has settled.
  pub(super) fn check_accept(&self, kind: &RunEventKind, execution: StepExecutionId) -> Result<(), RunError> {
    self.expect_step(kind, execution, |status| status == StepStatus::Failed)?;
    self.expect_settled()?;
    let failed = self.execution(execution).ok_or(RunError::UnknownExecution(execution))?;
    let latest = self.latest_execution(failed.step()).map(StepExecution::id) == Some(execution);
    if latest && failed.failure().is_some_and(StepFailure::waits_for_person) {
      Ok(())
    } else {
      Err(RunError::NotAcceptable(failed.step().clone()))
    }
  }

  /// The attempt after an acceptance executes every step that has not passed, the accepted one passed.
  pub(super) fn check_accepted(&self, step: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    if self.latest_execution(step).map(StepExecution::status) != Some(StepStatus::Passed) {
      return Err(RunError::InvalidAttempt("an acceptance passes its step first"));
    }
    let expected = self.unpassed_executions();
    if executions
      .iter()
      .map(|planned| &planned.step)
      .eq(expected.iter().map(|planned| &planned.step))
    {
      Ok(())
    } else {
      Err(RunError::InvalidAttempt(
        "the attempt after an acceptance executes the steps that have not passed",
      ))
    }
  }

  /// Fresh executions of every step whose latest execution has not passed, in pipeline order.
  fn unpassed_executions(&self) -> Vec<PlannedExecution> {
    self
      .snapshot()
      .steps()
      .iter()
      .map(|step| step.key())
      .filter(|key| self.latest_execution(key).map(StepExecution::status) != Some(StepStatus::Passed))
      .map(|key| PlannedExecution {
        id: StepExecutionId::generate(),
        step: key.clone(),
      })
      .collect()
  }
}
