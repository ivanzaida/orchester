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

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
  answer::Answerer,
  attempt::StepExecution,
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  state::Run,
  status::{AttemptReason, RunStatus, StepFailure, StepStatus, Stuck},
};
use crate::{
  id::{AttemptId, StepExecutionId},
  resource::StepKey,
  revision::Revision,
  time::Timestamp,
};

/// Why a step waiting unverified or unchanged cannot be accepted now: the work that would pass with it is not the work
/// its result was for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum AcceptRefusal {
  /// The revision the result was for is not recorded, so what work it covers is unknown.
  #[error("the revision the accepted result was for is not recorded, so Orchester cannot tell what work it covers")]
  NotJudged,
  /// The work no longer builds on `revision`, the revision the result was for (its short form).
  #[error("the accepted step's work no longer builds on {revision}, the revision its result was for")]
  NotBuiltOn { revision: String },
  /// Uncommitted changes lie beside the work, which passing would take as the step's.
  #[error("uncommitted changes lie beside the accepted step's work, which accepting would pass as the step's")]
  Uncommitted,
  /// The step worked in a worktree of its own, which was removed while its branch remains: its work cannot be merged.
  #[error(
    "the accepted step's own worktree was removed while its branch {branch} remains, so its work cannot be merged"
  )]
  WorktreeRemoved { branch: String },
}

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

  /// [`super::StuckDecision::Accept`] of a run waiting for `stuck`, decided `by` a person or by automation: the
  /// acceptance is recorded, and a new attempt runs the step, then the steps after it and those that have not passed.
  /// The step's new execution runs no agent: the runner passes it with the work and the result of the one accepted
  /// ([`Run::accepted_execution`]), as a step passes, merging its work and proposing a gated step's plan.
  pub(super) fn accept_stuck(
    &mut self,
    stuck: &Stuck,
    by: Answerer,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
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
      transition.record(RunEventKind::StepAccepted { execution, by })?;
      let attempt = RunEventKind::AttemptStarted {
        attempt: AttemptId::generate(),
        number: transition.run.next_attempt_number(),
        executions: transition.run.retry_executions(&step),
        reason: AttemptReason::Accepted { step },
      };
      transition.record(attempt)
    })
  }

  /// An acceptance names the latest execution of its step, which failed unverified or unchanged, once the attempt it
  /// ended has settled. It changes no execution: the attempt after it passes the step.
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

  /// The attempt after an acceptance executes the accepted step, which failed unverified or unchanged, its dependent
  /// steps and every step that has not passed.
  pub(super) fn check_accepted(&self, step: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    let waited = self
      .latest_execution(step)
      .is_some_and(|execution| execution.failure().is_some_and(StepFailure::waits_for_person));
    if !waited {
      return Err(RunError::InvalidAttempt(
        "an acceptance passes a step that waited for a person",
      ));
    }
    let expected = self.retry_executions(step);
    if executions
      .iter()
      .map(|planned| &planned.step)
      .eq(expected.iter().map(|planned| &planned.step))
    {
      Ok(())
    } else {
      Err(RunError::InvalidAttempt(
        "the attempt after an acceptance executes the accepted step, its dependent and its unfinished steps",
      ))
    }
  }

  /// The execution whose work and result a person accepted, when `execution` is the accepted step's execution in the
  /// attempt the acceptance started: the step's latest execution that failed unverified, or for one that was not run
  /// again on an unchanged revision, the one whose result held for it. The runner passes `execution` with that one's
  /// work and result instead of starting an agent.
  pub fn accepted_execution(&self, execution: StepExecutionId) -> Option<StepExecutionId> {
    let position = self
      .attempts()
      .iter()
      .position(|attempt| attempt.execution(execution).is_some())?;
    let attempt = &self.attempts()[position];
    let AttemptReason::Accepted { step } = attempt.reason() else {
      return None;
    };
    if attempt.execution(execution)?.step() != step {
      return None;
    }
    self.judged_work(step, position).map(StepExecution::id)
  }

  /// The execution an acceptance of the run, which waits for a person unverified or unchanged, would accept the
  /// work and result of; `None` for a run that does not wait so. The runner checks that work before it records the
  /// acceptance.
  pub fn acceptable(&self) -> Option<&StepExecution> {
    let RunStatus::WaitingForPerson(stuck) = self.status() else {
      return None;
    };
    if !stuck.can_accept() {
      return None;
    }
    self.judged_work(stuck.step(), self.attempts().len())
  }

  /// The latest execution of `step` in the attempts before `before` whose result held for its revision. An execution
  /// refused as unchanged did no work: the one whose result held for the revision is it.
  fn judged_work(&self, step: &StepKey, before: usize) -> Option<&StepExecution> {
    self.attempts()[..before]
      .iter()
      .rev()
      .filter(|earlier| earlier.counts())
      .flat_map(|earlier| earlier.executions())
      .filter(|candidate| candidate.step() == step && candidate.status() == StepStatus::Failed)
      .find(|candidate| candidate.failure() != Some(StepFailure::Unchanged))
      .filter(|accepted| accepted.failure().is_some_and(StepFailure::holds_for_revision))
  }
}
