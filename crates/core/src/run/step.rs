//! Step transitions: pending → running → waiting (approval, plan, message) → passed / failed /
//! skipped, with the plan approval gate and failure routing to an upstream step.

use super::{
  answer::Answerer,
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  plan::{Plan, PlanDecision, PlanReview},
  state::Run,
  status::{StepFailure, StepStatus, WaitReason},
};
use crate::{id::StepExecutionId, resource::StepKey, revision::Revision, time::Timestamp};

impl Run {
  /// Starts a pending execution of the current attempt whose dependencies have passed.
  pub fn start_step(&mut self, execution: StepExecutionId, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::StepStarted { execution })
    })
  }

  /// A running step waits for a tool access approval or a message. Waiting for a plan decision
  /// happens through [`Run::propose_plan`].
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

  /// A running step without an approval gate finished its work: it passes, and the run succeeds
  /// once every step has passed. A gated step finishes through [`Run::propose_plan`].
  pub fn complete_step(&mut self, execution: StepExecutionId, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::StepPassed { execution })?;
      transition.succeed_if_done()
    })
  }

  /// A running gated step finished its work: `text`, its output, becomes the step's next plan —
  /// numbered after the step's plans among `earlier`, the run's plans so far — and the step waits
  /// for a person's decision on it. Returns the events and the plan to store with them.
  pub fn propose_plan(
    &mut self,
    execution: StepExecutionId,
    text: String,
    earlier: &[Plan],
    now: Timestamp,
  ) -> Result<(Vec<RunEvent>, Plan), RunError> {
    let step = self
      .execution(execution)
      .ok_or(RunError::UnknownExecution(execution))?
      .step()
      .clone();
    let events = self.transition(now, |transition| {
      transition.record(RunEventKind::StepWaiting {
        execution,
        reason: WaitReason::Plan,
      })
    })?;
    let plan = Plan {
      run: self.id,
      execution,
      version: Plan::next_version(earlier, self.id, &step),
      step,
      text,
      created_at: now,
      review: None,
    };
    Ok((events, plan))
  }

  /// Records `by`'s `decision` on `plan`, the latest plan of a gated step that waits for one.
  /// Approval passes the step; requested changes send it back to its agent, and its revised result
  /// is a new plan that needs a new decision. `plan` gets the decision on success.
  pub fn decide_plan(
    &mut self,
    plan: &mut Plan,
    decision: PlanDecision,
    by: Answerer,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    if plan.run != self.id {
      return Err(RunError::UnknownExecution(plan.execution));
    }
    if plan.review.is_some() {
      return Err(RunError::PlanAlreadyDecided(plan.version));
    }
    let events = self.transition(now, |transition| {
      transition.record(RunEventKind::PlanDecided {
        execution: plan.execution,
        version: plan.version,
        decision: decision.clone(),
      })?;
      transition.succeed_if_done()
    })?;
    plan.review = Some(PlanReview { decision, by, at: now });
    Ok(events)
  }

  /// Fails an active step for `reason` and routes the failure ([`Run::after_failure`]): a verdict
  /// follows the step's failure route while it has retries left, a breakage restarts the step once,
  /// and with no route left the run fails or waits for a person. Open executions of the ending
  /// attempt — steps running in parallel included — are skipped either way, so their agents must be
  /// ended. A follow-up that fails is not routed: the run is succeeded again with the result it had.
  pub fn fail_step(
    &mut self,
    execution: StepExecutionId,
    reason: StepFailure,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    self.fail_judged(execution, (reason, None), now)
  }

  /// [`Run::fail_step`] for a result that was for `revision` of the run's branch, the one the execution
  /// started on: a step whose result holds for its revision is not run again on it
  /// ([`Run::reruns_unchanged`]).
  pub fn fail_judged(
    &mut self,
    execution: StepExecutionId,
    (reason, revision): (StepFailure, Option<Revision>),
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    let follow_up = self.in_follow_up();
    self.transition(now, |transition| {
      transition.record(RunEventKind::StepFailed {
        execution,
        reason: Some(reason),
        revision,
      })?;
      transition.skip_open()?;
      if follow_up {
        return transition.record(RunEventKind::FollowUpFailed);
      }
      let step = transition
        .run
        .execution(execution)
        .expect("StepFailed was accepted for this execution")
        .step()
        .clone();
      let next = transition.run.after_failure(&step, reason);
      transition.record(next)
    })
  }

  /// Whether `execution`'s result waits for a plan decision: its step has an approval gate, and it
  /// is no follow-up (the step's plan was decided before the run succeeded).
  pub fn is_gated(&self, execution: StepExecutionId) -> Result<bool, RunError> {
    let step = self
      .execution(execution)
      .ok_or(RunError::UnknownExecution(execution))?
      .step();
    let gate = self
      .snapshot()
      .body()
      .step(step)
      .expect("executions name snapshot steps")
      .approval_gate;
    Ok(gate && !self.is_follow_up(execution))
  }

  /// Executions of a retry attempt returning to `return_to`: that step, everything downstream
  /// of it, and any other step that has not passed. Passed steps elsewhere keep their result.
  pub(super) fn retry_executions(&self, return_to: &StepKey) -> Vec<PlannedExecution> {
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
