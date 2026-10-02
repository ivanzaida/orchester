//! What happens after a step fails, and what a person decides when no automatic route is left (user
//! decisions 2026-10-02). A verdict on the work (the agent reported `failed`, the command's exit code
//! said so) follows the step's failure route; once its retries are used up the run waits for a person.
//! Any other failure is a breakage: the step restarts once as a fresh execution, and breaking again
//! in that restart makes the run wait for a person too. Breakages never return to an upstream step.
//!
//! A run waiting for a person goes on when they allow one more retry (the same route, or the same
//! restart), return the work to an agent step with a note, or stop it, which fails it as before.
//! A retry attempt's prompt reports the failure that sent the work back ([`Run::retry_report`]).

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use super::{
  attempt::StepExecution,
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  plan::ReviewNote,
  snapshot::StepExecutor,
  state::Run,
  status::{AttemptReason, RunFailure, RunStatus, StepFailure, StepStatus, Stuck},
};
use crate::{
  id::{AttemptId, StepExecutionId},
  resource::StepKey,
  time::Timestamp,
};

/// A person's decision on a run that waits for them ([`RunStatus::WaitingForPerson`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum StuckDecision {
  /// One more attempt: the step's failure route again, or for a step that broke, the step again.
  Retry,
  /// A new attempt from agent step `step` (one of [`Run::return_targets`]), whose prompt carries
  /// `note` as the person's instruction.
  ReturnTo { step: StepKey, note: ReviewNote },
  /// The run fails, as it did before runs waited for a person.
  Stop,
}

/// The failure a retry attempt's execution is told about: the execution that failed and sent the
/// work back, the attempt it failed in, and the person's note when one returned the work to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryReportSource<'a> {
  pub failed: &'a StepExecution,
  pub attempt: NonZeroU32,
  pub note: Option<&'a ReviewNote>,
}

impl Run {
  /// What follows the failure of `step` for `reason` in an attempt that is not a follow-up.
  pub(super) fn after_failure(&self, step: &StepKey, reason: StepFailure) -> RunEventKind {
    if !reason.is_verdict() {
      return if self.restarts(step) {
        RunEventKind::WaitingForPerson {
          reason: Stuck::BrokeAgain(step.clone()),
        }
      } else {
        self.attempt(AttemptReason::Restart { step: step.clone() }, step)
      };
    }
    let route = self
      .snapshot()
      .body()
      .step(step)
      .expect("executions name snapshot steps")
      .on_failure
      .clone();
    match route {
      None => RunEventKind::Failed {
        reason: RunFailure::StepFailed(step.clone()),
      },
      Some(route) if self.retries_taken(step) >= route.max_retries.get() => RunEventKind::WaitingForPerson {
        reason: Stuck::RetriesExhausted(step.clone()),
      },
      Some(route) => self.attempt(
        AttemptReason::Retry {
          failed_step: step.clone(),
          return_to: route.return_to.clone(),
        },
        &route.return_to,
      ),
    }
  }

  /// Whether the current attempt is the restart of `step` after it broke.
  fn restarts(&self, step: &StepKey) -> bool {
    self.current_attempt().is_some_and(
      |attempt| matches!(attempt.reason(), AttemptReason::Restart { step: restarted } if restarted == step),
    )
  }

  /// A new attempt for `reason` that executes `from` again and everything after it.
  fn attempt(&self, reason: AttemptReason, from: &StepKey) -> RunEventKind {
    RunEventKind::AttemptStarted {
      attempt: AttemptId::generate(),
      number: self.next_attempt_number(),
      executions: self.retry_executions(from),
      reason,
    }
  }

  /// The agent steps a person may return the work of a waiting run to, in pipeline order: the step
  /// that failed and every agent step it runs after, each of which has executed. Empty for a run
  /// that does not wait for a person.
  pub fn return_targets(&self) -> Vec<StepKey> {
    let RunStatus::WaitingForPerson(stuck) = self.status() else {
      return Vec::new();
    };
    self
      .snapshot()
      .steps()
      .iter()
      .filter(|step| matches!(step.executor(), StepExecutor::Agent { .. }))
      .map(|step| step.key())
      .filter(|key| self.snapshot().downstream_of(key).contains(stuck.step()))
      .filter(|key| self.latest_execution(key).is_some())
      .cloned()
      .collect()
  }

  /// Carries out a person's `decision` on a run that waits for them.
  pub fn decide_stuck(&mut self, decision: &StuckDecision, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    let RunStatus::WaitingForPerson(stuck) = self.status().clone() else {
      return Err(RunError::InvalidRunTransition {
        status: self.status().clone(),
        event: RunEventKind::RetryAllowed.name(),
      });
    };
    let step = stuck.step().clone();
    let (reason, from) = match decision {
      StuckDecision::Stop => {
        return self.transition(now, |transition| {
          transition.record(RunEventKind::Failed {
            reason: stuck.failure(),
          })
        });
      }
      StuckDecision::Retry => match &stuck {
        Stuck::BrokeAgain(_) => (AttemptReason::Restart { step: step.clone() }, step),
        Stuck::RetriesExhausted(_) => {
          let return_to = self
            .snapshot()
            .body()
            .step(&step)
            .and_then(|pinned| pinned.on_failure.as_ref())
            .map(|route| route.return_to.clone())
            .ok_or(RunError::NoFailureRoute(step.clone()))?;
          let reason = AttemptReason::Retry {
            failed_step: step,
            return_to: return_to.clone(),
          };
          (reason, return_to)
        }
      },
      StuckDecision::ReturnTo { step: target, note } => {
        if !self.return_targets().contains(target) {
          return Err(RunError::NotAReturnTarget(target.clone()));
        }
        let reason = AttemptReason::Returned {
          failed_step: step,
          return_to: target.clone(),
          note: note.clone(),
        };
        (reason, target.clone())
      }
    };
    self.transition(now, |transition| {
      transition.record(RunEventKind::RetryAllowed)?;
      let attempt = transition.run.attempt(reason, &from);
      transition.record(attempt)
    })
  }

  /// A run fails from work (its last step failed) or when a person stops it while it waits for them,
  /// for the reason it waits.
  pub(super) fn check_failed(&self, kind: &RunEventKind, reason: &RunFailure) -> Result<(), RunError> {
    match self.status() {
      RunStatus::WaitingForPerson(stuck) => self.expect_status(kind, stuck.failure() == *reason),
      status => {
        self.expect_status(kind, *status == RunStatus::Running)?;
        self.expect_settled()
      }
    }
  }

  /// A restart executes a step whose latest execution broke, and everything after it.
  pub(super) fn check_restart(&self, step: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    let broke = self.latest_execution(step).is_some_and(|execution| {
      execution.status() == StepStatus::Failed && !execution.failure().is_some_and(StepFailure::is_verdict)
    });
    if !broke {
      return Err(RunError::InvalidAttempt("a restart executes a step that broke"));
    }
    self.expect_retry_executions(step, executions)
  }

  /// Work a person returned executes an agent step and everything after it.
  pub(super) fn check_returned(&self, return_to: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    if !matches!(
      self.snapshot().step(return_to).map(|step| step.executor()),
      Some(StepExecutor::Agent { .. })
    ) {
      return Err(RunError::InvalidAttempt("work is returned to an agent step"));
    }
    self.expect_retry_executions(return_to, executions)
  }

  fn expect_retry_executions(&self, from: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    let expected = self.retry_executions(from);
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

  /// The failure the prompt of `execution` reports, when it belongs to an attempt that a failure
  /// sent back: the step the work returned to learns why, and the step that failed gets its own
  /// earlier result to check against. Any other execution reports none.
  pub fn retry_report(&self, execution: StepExecutionId) -> Option<RetryReportSource<'_>> {
    let position = self
      .attempts()
      .iter()
      .position(|attempt| attempt.execution(execution).is_some())?;
    let attempt = &self.attempts()[position];
    let (failed_step, return_to, note) = match attempt.reason() {
      AttemptReason::Retry { failed_step, return_to } => (failed_step, return_to, None),
      AttemptReason::Returned {
        failed_step,
        return_to,
        note,
      } => (failed_step, return_to, Some(note)),
      _ => return None,
    };
    let step = attempt.execution(execution)?.step();
    if step != failed_step && step != return_to {
      return None;
    }
    let (earlier, failed) = self.attempts()[..position].iter().rev().find_map(|earlier| {
      earlier
        .executions()
        .iter()
        .find(|candidate| candidate.step() == failed_step && candidate.status() == StepStatus::Failed)
        .map(|failed| (earlier, failed))
    })?;
    Some(RetryReportSource {
      failed,
      attempt: earlier.number(),
      note: note.filter(|_| step == return_to),
    })
  }
}
