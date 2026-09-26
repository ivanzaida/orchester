//! Step transitions: pending → running → waiting (approval, plan, message) → passed / failed /
//! skipped, with the plan approval gate and failure routing to an upstream step.

use super::{
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  state::Run,
  status::{AttemptReason, PlanDecision, RunFailure, StepFailure, StepStatus, WaitReason},
};
use crate::{
  id::{AttemptId, StepExecutionId},
  resource::StepKey,
  time::Timestamp,
};

impl Run {
  /// Starts a pending execution of the current attempt whose dependencies have passed.
  pub fn start_step(&mut self, execution: StepExecutionId, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::StepStarted { execution })
    })
  }

  /// A running step waits for a tool access approval or a message. Waiting for a plan decision
  /// happens through [`Run::complete_step`] on a gated step.
  pub fn wait_step(
    &mut self,
    execution: StepExecutionId,
    reason: WaitReason,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::StepWaiting { execution, reason })
    })
  }

  /// A step waiting for an approval or a message works again.
  pub fn resume_step(&mut self, execution: StepExecutionId, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::StepResumed { execution })
    })
  }

  /// A running step finished its work. A gated step waits for a plan decision; any other step
  /// passes, and the run succeeds once every step has passed.
  pub fn complete_step(&mut self, execution: StepExecutionId, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    let gated = self.gated(execution)?;
    self.transition(now, |transition| {
      if gated {
        return transition.record(RunEventKind::StepWaiting {
          execution,
          reason: WaitReason::Plan,
        });
      }
      transition.record(RunEventKind::StepPassed { execution })?;
      transition.succeed_if_done()
    })
  }

  /// Records a person's decision on a gated step's result. Approval passes the step; requested
  /// changes send it back to work, and its revised result needs a new decision.
  pub fn decide_plan(
    &mut self,
    execution: StepExecutionId,
    decision: PlanDecision,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::PlanDecided { execution, decision })?;
      transition.succeed_if_done()
    })
  }

  /// Fails an active step for `reason` and routes the failure: with a failure route that has
  /// retries left, a new attempt re-executes the route's target and everything downstream of it;
  /// otherwise the run fails. Open executions of the ending attempt — steps running in parallel
  /// included — are skipped either way, so their agents must be ended.
  pub fn fail_step(
    &mut self,
    execution: StepExecutionId,
    reason: StepFailure,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::StepFailed {
        execution,
        reason: Some(reason),
      })?;
      transition.skip_open()?;
      let run = &transition.run;
      let step = run
        .execution(execution)
        .expect("StepFailed was accepted for this execution")
        .step()
        .clone();
      let route = run
        .snapshot()
        .body()
        .step(&step)
        .expect("executions name snapshot steps")
        .on_failure
        .clone();
      let next = match route {
        None => RunEventKind::Failed {
          reason: RunFailure::StepFailed(step),
        },
        Some(route) if run.retries_taken(&step) >= route.max_retries.get() => RunEventKind::Failed {
          reason: RunFailure::RetriesExhausted(step),
        },
        Some(route) => RunEventKind::AttemptStarted {
          attempt: AttemptId::generate(),
          number: run.next_attempt_number(),
          executions: run.retry_executions(&route.return_to),
          reason: AttemptReason::Retry {
            failed_step: step,
            return_to: route.return_to,
          },
        },
      };
      transition.record(next)
    })
  }

  fn gated(&self, execution: StepExecutionId) -> Result<bool, RunError> {
    let step = self
      .execution(execution)
      .ok_or(RunError::UnknownExecution(execution))?
      .step();
    Ok(
      self
        .snapshot()
        .body()
        .step(step)
        .expect("executions name snapshot steps")
        .approval_gate,
    )
  }

  /// Executions of a retry attempt returning to `return_to`: that step, everything downstream
  /// of it, and any other step that has not passed. Passed steps elsewhere keep their result.
  fn retry_executions(&self, return_to: &StepKey) -> Vec<PlannedExecution> {
    let reset = self.snapshot().downstream_of(return_to);
    self
      .snapshot()
      .steps()
      .iter()
      .map(|step| step.key())
      .filter(|key| {
        reset.contains(*key)
          || self.latest_execution(key).map(|execution| execution.status()) != Some(StepStatus::Passed)
      })
      .map(|key| PlannedExecution {
        id: StepExecutionId::generate(),
        step: key.clone(),
      })
      .collect()
  }
}

impl super::apply::Transition {
  fn succeed_if_done(&mut self) -> Result<(), RunError> {
    if self.run.all_passed() {
      self.record(RunEventKind::Succeeded)
    } else {
      Ok(())
    }
  }
}
