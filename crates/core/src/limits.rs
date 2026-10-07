//! Project settings › Limits (02.M `c7e729`): budgets are project resources, so their bounds and
//! defaults are project settings. Changes apply to new runs; a run keeps the limits it started
//! with, and a raised limit is bounded by the maximum in force when it is raised. How many agents
//! work at once is the exception: the project's runner applies it at once.

use std::{num::NonZeroU32, time::Duration};

use thiserror::Error;

use crate::tokens::{TokenBudget, TokensUsed};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ProjectLimitsError {
  #[error("the default run budget {default} is above the maximum run budget {max}")]
  DefaultAboveMaximum { default: TokenBudget, max: TokenBudget },
  #[error("the default time limit must be greater than zero")]
  ZeroTimeLimit,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("{count} agents at once is outside {min} to {max}", min = AgentSlots::MIN, max = AgentSlots::MAX)]
pub struct AgentSlotsError {
  pub count: u64,
}

/// How many agents the project's runner lets work at once: each agent at work in any of its runs
/// takes one slot; a command step takes a command slot instead. Runs beyond them wait in the queue
/// with that reason. Lowering it stops no agent: new work waits until fewer agents work than it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentSlots(NonZeroU32);

impl AgentSlots {
  pub const MIN: u32 = 1;
  pub const MAX: u32 = 16;
  pub const DEFAULT: Self = Self(NonZeroU32::new(4).expect("four is above zero"));

  pub fn new(count: u64) -> Result<Self, AgentSlotsError> {
    u32::try_from(count)
      .ok()
      .filter(|count| (Self::MIN..=Self::MAX).contains(count))
      .and_then(NonZeroU32::new)
      .map(Self)
      .ok_or(AgentSlotsError { count })
  }

  pub const fn get(self) -> NonZeroU32 {
    self.0
  }
}

impl Default for AgentSlots {
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// A project's token budgets, time limit default and agent slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectLimits {
  max_run_budget: TokenBudget,
  default_run_budget: TokenBudget,
  daily_budget: TokenBudget,
  default_time_limit: Duration,
  agent_slots: AgentSlots,
}

impl ProjectLimits {
  /// The design's defaults: at most 10M tokens per run, 1.4M by default, 20M per project per
  /// day, 30 minutes of active agent time and 4 agents at once.
  pub const DEFAULT: Self = Self {
    max_run_budget: budget(10_000_000),
    default_run_budget: budget(1_400_000),
    daily_budget: budget(20_000_000),
    default_time_limit: Duration::from_secs(30 * 60),
    agent_slots: AgentSlots::DEFAULT,
  };

  /// The budgets and the time limit, with the default agent slots ([`ProjectLimits::with_agent_slots`]).
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
      agent_slots: AgentSlots::DEFAULT,
    })
  }

  #[must_use]
  pub const fn with_agent_slots(mut self, agent_slots: AgentSlots) -> Self {
    self.agent_slots = agent_slots;
    self
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

  /// How many agents may work at once.
  pub const fn agent_slots(self) -> AgentSlots {
    self.agent_slots
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
