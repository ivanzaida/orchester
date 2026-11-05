//! The usage area of the coordinator link: the tokens a step's executions used toward their step's budget, over every
//! invocation of theirs and the reviews of their requests ([`orchester_core::run::execution_tokens`]), as the project
//! has them recorded. Asked where they were read before: as an agent starts, and as the run looks whether one of its
//! running steps used up its budget.

use orchester_core::{
  id::{RunId, StepExecutionId},
  tokens::TokensUsed,
};
use serde::{Deserialize, Serialize};

use super::LinkError;

/// The coordinator's answer about what a run's steps used.
pub trait UsageLink {
  /// The tokens each execution `request` names used toward its step's budget.
  fn step_usage(&self, request: &StepUsageRequest) -> Result<StepUsage, LinkError>;
}

/// What `executions` of `run` used, read together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepUsageRequest {
  pub run: RunId,
  pub executions: Vec<StepExecutionId>,
}

/// What each execution asked for used, in the order they were asked.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepUsage {
  pub used: Vec<ExecutionUsage>,
}

/// The tokens one execution used toward its step's budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionUsage {
  pub execution: StepExecutionId,
  pub tokens: TokensUsed,
}

impl StepUsage {
  /// What `execution` used; none when the answer does not name it.
  pub fn of(&self, execution: StepExecutionId) -> TokensUsed {
    self
      .used
      .iter()
      .find(|used| used.execution == execution)
      .map_or_else(TokensUsed::default, |used| used.tokens)
  }
}

/// What `executions` of `run` used toward their steps' budgets, asked through `link`.
pub fn step_usage(
  link: &(impl UsageLink + ?Sized),
  run: RunId,
  executions: Vec<StepExecutionId>,
) -> Result<StepUsage, LinkError> {
  link.step_usage(&StepUsageRequest { run, executions })
}
