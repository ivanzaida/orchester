//! What a run waits for a person on, step by step: a plan, an agent's question, or a decision after a
//! step's retries ran out, as the task list's rows and every *Needs you* name them.

use std::sync::Arc;

use orchester_core::{
  id::StepExecutionId,
  resource::StepKey,
  run::{Plan, PlanVersion, Run, RunStatus, Stuck},
};

use super::standing::CurrentStep;
use crate::messages::Message;

/// What a run waiting for a person's decision stopped at (02.B6 `Np9kc`): the step, by name, and
/// whether its retries ran out (how many it took) or it broke again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaitedDecision {
  pub step: Arc<str>,
  pub retries: u32,
  pub stuck: Stuck,
}

impl AwaitedDecision {
  /// What `run` waits for a decision on, if it does.
  pub fn of(run: &Run) -> Option<Self> {
    let RunStatus::WaitingForPerson(stuck) = run.status() else {
      return None;
    };
    let step = stuck.step();
    Some(Self {
      step: run
        .snapshot()
        .body()
        .step(step)
        .map_or_else(|| step.as_str().into(), |pinned| pinned.name.as_str().into()),
      retries: run.automatic_retries_taken(step),
      stuck: stuck.clone(),
    })
  }

  /// "Review still fails after 2 retries", "Implement stopped again without a result", "Verify could not
  /// check everything" or "Verify was not run again: nothing changed".
  pub fn title(&self) -> Message {
    let key = match (&self.stuck, self.retries) {
      (Stuck::BrokeAgain(_), _) => "runs.retryLimit.brokeTitle",
      (Stuck::Unverified(_), _) => "runs.retryLimit.unverifiedTitle",
      (Stuck::Unchanged(_), _) => "runs.retryLimit.unchangedTitle",
      (Stuck::RetriesExhausted(_), 1) => "runs.retryLimit.title.one",
      (Stuck::RetriesExhausted(_), _) => "runs.retryLimit.title.other",
    };
    Message::new(key)
      .with("step", self.step.to_string())
      .with("retries", self.retries.to_string())
  }
}

/// A question an agent asked, which its run waits for an answer to: who asks (the agent role of
/// its step, the step's name when its profile version is gone) and what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaitedQuestion {
  pub agent: Arc<str>,
  pub text: Arc<str>,
}

/// A step of a run that waits for a person: on its plan, or for the answer to its agent's question.
/// Parallel steps may wait at the same time, each for its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaitedStep {
  pub execution: StepExecutionId,
  /// The step, as people name it.
  pub step: Option<CurrentStep>,
  pub wait: StepWait,
}

/// What a waiting step waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepWait {
  Plan(AwaitedPlan),
  /// An answer to its agent's question; `None` when no question of it can be read.
  Answer(Option<AwaitedQuestion>),
}

/// A plan a run waits on a decision for (02.11): its version and who proposed it, the agent role
/// of its step (the step's name when its profile version is gone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaitedPlan {
  pub version: PlanVersion,
  pub agent: Arc<str>,
}

impl AwaitedPlan {
  /// The plan `run`'s `execution` waits on, among its `plans`, proposed by `agent`'s step.
  pub fn of(
    run: &Run,
    plans: &[Plan],
    execution: StepExecutionId,
    agent: impl Fn(&StepKey) -> Option<Arc<str>>,
  ) -> Option<Self> {
    let plan = Plan::latest_of(plans, execution)?;
    let step = run.snapshot().body().step(&plan.step)?;
    Some(Self {
      version: plan.version,
      agent: agent(&plan.step).unwrap_or_else(|| step.name.as_str().into()),
    })
  }
}
