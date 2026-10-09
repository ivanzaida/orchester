//! Raising the limit a run stopped at so it continues (02.L `ZUYM0`): what the run used of the limit
//! that stopped it, the new budget and time limit read as the task page's panel reads them, and the
//! runner request that applies them. Orchester's MCP server reads its arguments through the same
//! rules.

use std::time::Duration;

use orchester_core::{
  id::RunId,
  run::{RaisedLimits, RunLimit},
  tokens::{TokenBudget, TokensUsed, compact_tokens},
};
use orchester_ipc::Command;

use super::detail::RunView;
use crate::{
  budget::{read_minutes, read_optional_budget, whole_minutes},
  messages::Message,
};

/// What continuing raises: the run's own limits, or the budget of the step it stopped at. `None`
/// lifts a limit: an empty field is no limit.
#[derive(Clone, Copy, Debug)]
pub enum Raise {
  Limits(RaisedLimits),
  StepBudget(Option<TokenBudget>),
}

impl Raise {
  /// The runner request that raises the limit of `run` and lets it continue.
  pub fn command(self, run: RunId) -> Command {
    match self {
      Self::Limits(limits) => Command::RaiseLimit {
        run,
        budget: limits.budget,
        // Limits are whole minutes; one beyond u64 milliseconds is as good as no limit.
        max_duration_ms: limits
          .max_duration
          .map(|limit| u64::try_from(limit.as_millis()).unwrap_or(u64::MAX)),
      },
      Self::StepBudget(budget) => Command::RaiseStepBudget { run, budget },
    }
  }
}

/// What `run` used of the budget that stopped it at `limit`, and that budget (`None` when the run
/// has none, as after its time limit): after a step's budget, of that step's execution. `None` for
/// a group's budget, which the run does not raise, and for a step without a budget of its own,
/// which no runner records.
pub fn budget_use(run: &RunView, limit: RunLimit) -> Option<(TokensUsed, Option<TokenBudget>)> {
  match limit {
    RunLimit::GroupBudget => None,
    RunLimit::StepBudget { execution } => run
      .step_budget_use(execution)
      .map(|(used, budget)| (used, Some(budget))),
    RunLimit::Tokens | RunLimit::Time | RunLimit::DailyBudget => Some((run.run.used(), run.run.budget())),
  }
}

/// The run's new token budget in `text`: above what it `used`, or no budget when empty.
pub fn run_budget(text: &str, separator: &str, used: TokensUsed) -> Result<Option<TokenBudget>, Message> {
  let budget = read_optional_budget(text, separator)?;
  if budget.is_some_and(|budget| used.reached(budget)) {
    return Err(Message::new("tokens.errors.notAboveUsed").with("used", compact_tokens(used.get())));
  }
  Ok(budget)
}

/// The new budget of the run's `step` in `text`: above what the step's execution `used`, or no
/// budget when empty.
pub fn step_budget(text: &str, separator: &str, used: TokensUsed, step: &str) -> Result<Option<TokenBudget>, Message> {
  let budget = read_optional_budget(text, separator)?;
  if budget.is_some_and(|budget| used.reached(budget)) {
    return Err(
      Message::new("tokens.errors.stepNotAboveUsed")
        .with("used", compact_tokens(used.get()))
        .with("step", step.to_owned()),
    );
  }
  Ok(budget)
}

/// The run's new time limit in whole minutes in `text`: above the `active` time it used, or no time
/// limit when empty.
pub fn time_limit(text: &str, active: Duration) -> Result<Option<Duration>, Message> {
  let limit = read_minutes(text)?;
  if limit.is_some_and(|limit| limit <= active) {
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
