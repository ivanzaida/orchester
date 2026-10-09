//! Project settings › Limits (02.M `c7e729`): budgets are project resources, so their defaults
//! are project settings. A run's budget and time limit are optional and have no maximum: an empty
//! one means no limit (user decision 2026-10-09). Changes apply to new runs; a run keeps the
//! limits it started with. How many agents work at once is the exception: the project's runner
//! applies it at once.

use std::{num::NonZeroU32, time::Duration};

use thiserror::Error;

use crate::tokens::{TokenBudget, TokensUsed};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ProjectLimitsError {
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
  default_run_budget: Option<TokenBudget>,
  daily_budget: TokenBudget,
  default_time_limit: Option<Duration>,
  agent_slots: AgentSlots,
}

impl ProjectLimits {
  /// No run budget and no time limit by default (user decision 2026-10-09), 20M tokens per
  /// project per day and 4 agents at once.
  pub const DEFAULT: Self = Self {
    default_run_budget: None,
    daily_budget: budget(20_000_000),
    default_time_limit: None,
    agent_slots: AgentSlots::DEFAULT,
  };

  /// The budgets and the time limit, with the default agent slots ([`ProjectLimits::with_agent_slots`]).
  /// `None` for the run budget or the time limit means new tasks have none.
  pub fn new(
    default_run_budget: Option<TokenBudget>,
    daily_budget: TokenBudget,
    default_time_limit: Option<Duration>,
  ) -> Result<Self, ProjectLimitsError> {
    if default_time_limit.is_some_and(|limit| limit.is_zero()) {
      return Err(ProjectLimitsError::ZeroTimeLimit);
    }
    Ok(Self {
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

  /// Prefilled in New task; `None` leaves new tasks without a budget.
  pub const fn default_run_budget(self) -> Option<TokenBudget> {
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

  /// Prefilled in New task; counts active agent time only. `None` leaves new tasks without a
  /// time limit.
  pub const fn default_time_limit(self) -> Option<Duration> {
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
