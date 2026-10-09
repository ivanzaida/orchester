//! The plan review of a task whose run waits for a plan decision (02.11 `x3lVC`): the plan that
//! waits and the earlier versions of its step, who proposed them, and whose work approving starts.

use std::sync::Arc;

use orchester_core::{
  id::RunId,
  run::{Answerer, Plan, PlanDecision, PlanVersion, RunStatus},
};

use super::detail::RunView;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanReview {
  pub run: RunId,
  /// The gated step, and its agent role (its name when its profile version is gone).
  pub step: Arc<str>,
  pub agent: Arc<str>,
  /// The agents of the steps that run after the gated step, in pipeline order, each once: approving
  /// starts them.
  pub next: Vec<Arc<str>>,
  /// The step's plans in this run, oldest first; the last one waits for the decision.
  plans: Vec<Plan>,
}

impl PlanReview {
  /// The review `run` waits for, when a step of it waits for a decision on its plan; the first in
  /// pipeline order when several do.
  pub fn of(run: &RunView) -> Option<Self> {
    if run.run.status() != &RunStatus::Running {
      return None;
    }
    let execution = *run.run.awaiting_plan().first()?;
    let waiting = Plan::latest_of(&run.plans, execution).filter(|plan| plan.review.is_none())?;
    let step = run.steps.iter().find(|step| step.key == waiting.step)?;
    let mut plans: Vec<Plan> = run.plans.iter().filter(|plan| plan.step == step.key).cloned().collect();
    plans.sort_by_key(|plan| plan.version);
    let mut next: Vec<Arc<str>> = Vec::new();
    for body in run.run.snapshot().body().steps() {
      let after = body.depends_on.contains(&step.key);
      let agent = run
        .steps
        .iter()
        .find(|view| view.key == body.key)
        .map(|view| view.agent.clone().unwrap_or_else(|| view.name.clone()));
      if let Some(agent) = agent.filter(|agent| after && !next.contains(agent)) {
        next.push(agent);
      }
    }
    Some(Self {
      run: run.id(),
      agent: step.agent.clone().unwrap_or_else(|| step.name.clone()),
      step: step.name.clone(),
      next,
      plans,
    })
  }

  /// The plan that waits for the decision.
  pub fn waiting(&self) -> &Plan {
    self
      .plans
      .last()
      .expect("a review is made only for a step with a plan that waits")
  }

  /// The step's plans, oldest first.
  pub fn plans(&self) -> &[Plan] {
    &self.plans
  }

  /// Plan `version` of the step, or the waiting one when there is none.
  pub fn shown(&self, version: Option<PlanVersion>) -> &Plan {
    version
      .and_then(|version| self.plans.iter().find(|plan| plan.version == version))
      .unwrap_or_else(|| self.waiting())
  }
}

/// Where a plan stands, as its version reads: "v1 · awaiting approval"; a decision automation made
/// says so ("v1 · approved by automation"), a person's reads plain.
pub fn state_key(plan: &Plan) -> &'static str {
  let Some(review) = &plan.review else {
    return "runs.plan.version.awaiting";
  };
  match (&review.decision, review.by) {
    (PlanDecision::Approved, Answerer::Person) => "runs.plan.version.approved",
    (PlanDecision::Approved, Answerer::Automation) => "runs.plan.version.approvedByAutomation",
    (PlanDecision::ChangesRequested { .. }, Answerer::Person) => "runs.plan.version.changesRequested",
    (PlanDecision::ChangesRequested { .. }, Answerer::Automation) => "runs.plan.version.changesRequestedByAutomation",
  }
}
