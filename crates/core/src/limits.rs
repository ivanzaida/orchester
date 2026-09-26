//! Project settings › Limits (02.M `c7e729`): budgets are project resources, so their bounds and
//! defaults are project settings. Changes apply to new runs; a run keeps the limits it started
//! with, and a raised limit is bounded by the maximum in force when it is raised.

use std::time::Duration;

use thiserror::Error;

use crate::tokens::{TokenBudget, TokensUsed};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ProjectLimitsError {
  #[error("the default run budget {default} is above the maximum run budget {max}")]
  DefaultAboveMaximum { default: TokenBudget, max: TokenBudget },
  #[error("the default time limit must be greater than zero")]
  ZeroTimeLimit,
}

/// A project's token budgets and time limit defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectLimits {
  max_run_budget: TokenBudget,
  default_run_budget: TokenBudget,
  daily_budget: TokenBudget,
  default_time_limit: Duration,
}

impl ProjectLimits {
  /// The design's defaults: at most 10M tokens per run, 1.4M by default, 20M per project per
  /// day and 30 minutes of active agent time.
  pub const DEFAULT: Self = Self {
    max_run_budget: budget(10_000_000),
    default_run_budget: budget(1_400_000),
    daily_budget: budget(20_000_000),
    default_time_limit: Duration::from_secs(30 * 60),
  };

  pub fn new(
    max_run_budget: TokenBudget,
    default_run_budget: TokenBudget,
    daily_budget: TokenBudget,
    default_time_limit: Duration,
  ) -> Result<Self, ProjectLimitsError> {
    if default_run_budget > max_run_budget {
      return Err(ProjectLimitsError::DefaultAboveMaximum {
        default: default_run_budget,
        max: max_run_budget,
      });
    }
    if default_time_limit.is_zero() {
      return Err(ProjectLimitsError::ZeroTimeLimit);
    }
    Ok(Self {
      max_run_budget,
      default_run_budget,
      daily_budget,
      default_time_limit,
    })
  }

  /// The largest budget a task, a run or a raised limit may have.
  pub const fn max_run_budget(self) -> TokenBudget {
    self.max_run_budget
  }

  /// Prefilled in New task.
  pub const fn default_run_budget(self) -> TokenBudget {
    self.default_run_budget
  }

  /// Tokens all runs of the project may use per day.
  pub const fn daily_budget(self) -> TokenBudget {
    self.daily_budget
  }

  /// Whether the project's runs used up today's budget with `used_today`: new runs wait and
  /// running ones stop at their next turn until the day has room again.
  pub const fn daily_budget_used(self, used_today: TokensUsed) -> bool {
    used_today.reached(self.daily_budget)
  }

  /// Prefilled in New task; counts active agent time only.
  pub const fn default_time_limit(self) -> Duration {
    self.default_time_limit
  }
}

impl Default for ProjectLimits {
  fn default() -> Self {
    Self::DEFAULT
  }
}

const fn budget(tokens: u64) -> TokenBudget {
  match TokenBudget::from_const(tokens) {
    Some(budget) => budget,
    None => panic!("a default budget is above zero"),
  }
}
