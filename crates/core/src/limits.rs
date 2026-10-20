//! Project settings › Limits (02.M `c7e729`): budgets are project resources, so their defaults
//! are project settings. A run's budget and time limit are optional and have no maximum: an empty
//! one means no limit (user decision 2026-10-09). Changes apply to new runs; a run keeps the
//! limits it started with. How many agents work at once is the exception: the project's runner
//! applies it at once; how many QA apps are open at once applies from the next QA app's start.

use std::{num::NonZeroU32, time::Duration};

use thiserror::Error;

use crate::tokens::{TokenBudget, TokensUsed};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ProjectLimitsError {
  #[error("the default time limit must be greater than zero")]
  ZeroTimeLimit,
  #[error("the default time limit can be at most {} milliseconds", LONGEST_TIME_LIMIT.as_millis())]
  TimeLimitTooLong,
}

/// The longest time limit a task, a run or a project default can have: what a store keeps as whole
/// milliseconds in a signed 64-bit integer, far beyond any run. Not a limit a person sets.
pub const LONGEST_TIME_LIMIT: Duration = Duration::from_millis(i64::MAX.unsigned_abs());

/// Whether `limit` is longer than any time limit can be ([`LONGEST_TIME_LIMIT`]).
pub(crate) fn too_long(limit: Option<Duration>) -> bool {
  limit.is_some_and(|limit| limit > LONGEST_TIME_LIMIT)
}

/// The fewest and the most slots a project can set, for agents and for QA apps alike.
const SLOTS: std::ops::RangeInclusive<u32> = 1..=16;

/// `count` when it lies in [`SLOTS`].
fn slots(count: u64) -> Option<NonZeroU32> {
  u32::try_from(count)
    .ok()
    .filter(|count| SLOTS.contains(count))
    .and_then(NonZeroU32::new)
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
  pub const MIN: u32 = *SLOTS.start();
  pub const MAX: u32 = *SLOTS.end();
  pub const DEFAULT: Self = Self(NonZeroU32::new(4).expect("four is above zero"));

  pub fn new(count: u64) -> Result<Self, AgentSlotsError> {
    slots(count).map(Self).ok_or(AgentSlotsError { count })
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

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("{count} QA apps at once is outside {min} to {max}", min = QaSlots::MIN, max = QaSlots::MAX)]
pub struct QaSlotsError {
  pub count: u64,
}

/// How many QA apps this project's runs may have open at once (`start_qa_app`): each open one takes one of the
/// computer's QA slots, which every project's runner shares; a start while all the slots it may use are taken waits
/// until one is free. Read at each start, so a change applies to the next one; lowering it closes no open app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QaSlots(NonZeroU32);

impl QaSlots {
  pub const MIN: u32 = *SLOTS.start();
  pub const MAX: u32 = *SLOTS.end();
  pub const DEFAULT: Self = Self(NonZeroU32::new(2).expect("two is above zero"));

  pub fn new(count: u64) -> Result<Self, QaSlotsError> {
    slots(count).map(Self).ok_or(QaSlotsError { count })
  }

  pub const fn get(self) -> NonZeroU32 {
    self.0
  }
}

impl Default for QaSlots {
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// A project's token budgets, time limit default, agent slots and QA slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectLimits {
  default_run_budget: Option<TokenBudget>,
  daily_budget: TokenBudget,
  daily_budget_unlimited: bool,
  default_time_limit: Option<Duration>,
  agent_slots: AgentSlots,
  qa_slots: QaSlots,
}

impl ProjectLimits {
  /// No run budget and no time limit by default (user decision 2026-10-09), 20M tokens per
  /// project per day, 4 agents and 2 QA apps at once.
  pub const DEFAULT: Self = Self {
    default_run_budget: None,
    daily_budget: budget(20_000_000),
    daily_budget_unlimited: false,
    default_time_limit: None,
    agent_slots: AgentSlots::DEFAULT,
    qa_slots: QaSlots::DEFAULT,
  };

  /// The budgets and the time limit, with the default agent slots ([`ProjectLimits::with_agent_slots`]) and QA
  /// slots ([`ProjectLimits::with_qa_slots`]). `None` for the run budget or the time limit means new tasks have none.
  pub fn new(
    default_run_budget: Option<TokenBudget>,
    daily_budget: TokenBudget,
    default_time_limit: Option<Duration>,
  ) -> Result<Self, ProjectLimitsError> {
    if default_time_limit.is_some_and(|limit| limit.is_zero()) {
      return Err(ProjectLimitsError::ZeroTimeLimit);
    }
    if too_long(default_time_limit) {
      return Err(ProjectLimitsError::TimeLimitTooLong);
    }
    Ok(Self {
      default_run_budget,
      daily_budget,
      daily_budget_unlimited: false,
      default_time_limit,
      agent_slots: AgentSlots::DEFAULT,
      qa_slots: QaSlots::DEFAULT,
    })
  }

  #[must_use]
  pub const fn with_agent_slots(mut self, agent_slots: AgentSlots) -> Self {
    self.agent_slots = agent_slots;
    self
  }

  #[must_use]
  pub const fn with_qa_slots(mut self, qa_slots: QaSlots) -> Self {
    self.qa_slots = qa_slots;
    self
  }

  /// Prefilled in New task; `None` leaves new tasks without a budget.
  pub const fn default_run_budget(self) -> Option<TokenBudget> {
    self.default_run_budget
  }

  /// Sets the daily limit without discarding the last finite budget, used by the legacy app.
  #[must_use]
  pub const fn with_daily_budget_limit(mut self, limit: Option<TokenBudget>) -> Self {
    self.daily_budget_unlimited = limit.is_none();
    if let Some(budget) = limit {
      self.daily_budget = budget;
    }
    self
  }

  /// Tokens all runs may use per day; `None` means no daily limit.
  pub const fn daily_budget_limit(self) -> Option<TokenBudget> {
    if self.daily_budget_unlimited {
      None
    } else {
      Some(self.daily_budget)
    }
  }

  /// The last finite daily budget, for callers of the legacy settings API.
  /// Enforcement uses `daily_budget_used`; new screens use `daily_budget_limit`.
  pub const fn daily_budget(self) -> TokenBudget {
    self.daily_budget
  }

  /// Whether the project's runs used up today's budget with `used_today`: new runs wait and
  /// running ones stop at their next turn until the day has room again.
  pub const fn daily_budget_used(self, used_today: TokensUsed) -> bool {
    !self.daily_budget_unlimited && used_today.reached(self.daily_budget)
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

  /// How many QA apps the project's runs may have open at once.
  pub const fn qa_slots(self) -> QaSlots {
    self.qa_slots
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
