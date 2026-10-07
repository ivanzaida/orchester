//! Raising the limit a run stopped at so it continues (02.L `ZUYM0`): what the run used of the limit
//! that stopped it, the new budget and time limit read as the task page's panel reads them, and the
//! runner request that applies them. Orchester's MCP server reads its arguments through the same
//! rules.

use std::time::Duration;

use orchester_core::{
  id::RunId,
  run::{RaisedLimits, RunLimit},
  tokens::{BudgetInputError, TokenBudget, TokensUsed, compact_tokens, parse_budget},
};
use orchester_ipc::Command;

use super::detail::RunView;
use crate::{
  budget::{budget_error, read_budget, read_minutes, whole_minutes},
  messages::Message,
};

/// What continuing raises: the run's own limits, or the budget of the step it stopped at.
#[derive(Clone, Copy, Debug)]
pub enum Raise {
  Limits(RaisedLimits),
  StepBudget(TokenBudget),
}

impl Raise {
  /// The runner request that raises the limit of `run` and lets it continue.
  pub fn command(self, run: RunId) -> Command {
    match self {
      Self::Limits(limits) => Command::RaiseLimit {
        run,
        budget: limits.budget,
        // Limits are whole minutes; one beyond u64 milliseconds is as good as no limit.
        max_duration_ms: u64::try_from(limits.max_duration.as_millis()).unwrap_or(u64::MAX),
      },
      Self::StepBudget(budget) => Command::RaiseStepBudget { run, budget },
    }
  }
}

/// What `run` used of the budget that stopped it at `limit`, and that budget: after a step's budget,
/// of that step's execution. `None` for a group's budget, which the run does not raise, and for a
/// step without a budget of its own, which no runner records.
pub fn budget_use(run: &RunView, limit: RunLimit) -> Option<(TokensUsed, TokenBudget)> {
  match limit {
    RunLimit::GroupBudget => None,
    RunLimit::StepBudget { execution } => run.step_budget_use(execution),
    RunLimit::Tokens | RunLimit::Time | RunLimit::DailyBudget => Some((run.run.used(), run.run.budget())),
  }
}

/// The run's new token budget in `text`: at most `max`, above what it `used`.
pub fn run_budget(text: &str, separator: &str, (used, max): (TokensUsed, TokenBudget)) -> Result<TokenBudget, Message> {
  let budget = read_budget(text, separator, max)?;
  if used.reached(budget) {
    return Err(Message::new("tokens.errors.notAboveUsed").with("used", compact_tokens(used.get())));
  }
  Ok(budget)
}

/// The new budget of the run's `step` in `text`: at most `max`, above what the step's execution
/// `used`.
pub fn step_budget(
  text: &str,
  separator: &str,
  (used, max): (TokensUsed, TokenBudget),
  step: &str,
) -> Result<TokenBudget, Message> {
  let budget = parse_budget(text, separator, max).map_err(|error| match error {
    BudgetInputError::AboveMaximum { max } => {
      Message::new("tokens.errors.stepAboveMax").with("max", compact_tokens(max.get()))
    }
    other => budget_error(other, separator),
  })?;
  if used.reached(budget) {
    return Err(
      Message::new("tokens.errors.stepNotAboveUsed")
        .with("used", compact_tokens(used.get()))
        .with("step", step.to_owned()),
    );
  }
  Ok(budget)
}

/// The run's new time limit in whole minutes in `text`: above the `active` time it used.
pub fn time_limit(text: &str, active: Duration) -> Result<Duration, Message> {
  let limit = read_minutes(text)?;
  if limit <= active {
    let used = whole_minutes(active);
    let key = if used == 1 {
      "errors.timeLimitNotAboveUsed.one"
    } else {
      "errors.timeLimitNotAboveUsed.other"
    };
    return Err(Message::new(key).with("used", used.to_string()));
  }
  Ok(limit)
}
