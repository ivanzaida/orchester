//! Token budgets and time limits as people enter and read them (`docs/design/orchester-workspace.md`,
//! "Token budgets", "Stopped at limit"): field errors, the Project settings › Limits form,
//! whole-minute time limits of active agent time and how many agents work at once.

use std::time::Duration;

use orchester_core::{
  limits::{AgentSlots, ProjectLimits, ProjectLimitsError},
  tokens::{BudgetInputError, TokenBudget, compact_tokens, grouped, parse_budget},
};

use crate::messages::Message;

const SECONDS_PER_MINUTE: u64 = 60;
/// The example in "Use digits only, for example 1,400,000.", grouped for the locale.
const EXAMPLE_BUDGET: u64 = 1_400_000;

/// Bound of budgets that are not run budgets (the daily project budget, the maximum itself).
pub fn unbounded() -> TokenBudget {
  TokenBudget::new(u64::MAX).expect("u64::MAX is above zero")
}

/// The largest token budget the project database holds: its columns are 64-bit signed integers.
pub fn largest_stored() -> TokenBudget {
  TokenBudget::new(i64::MAX.unsigned_abs()).expect("i64::MAX is above zero")
}

/// The longest time limit the project database holds, in whole minutes: it stores whole milliseconds in
/// a 64-bit signed integer.
pub const MAX_STORED_MINUTES: u64 = i64::MAX.unsigned_abs() / (SECONDS_PER_MINUTE * 1000);

/// What a person reads for a budget field's text that is not a budget.
pub fn budget_error(error: BudgetInputError, separator: &str) -> Message {
  match error {
    BudgetInputError::Empty => Message::new("tokens.errors.empty"),
    BudgetInputError::NotDigits => {
      Message::new("tokens.errors.digits").with("example", grouped(EXAMPLE_BUDGET, separator))
    }
    BudgetInputError::Zero => Message::new("tokens.errors.zero"),
    BudgetInputError::AboveMaximum { max } => above(max, separator),
  }
}

/// Why a budget above `max` is not taken: the project's maximum run budget, or for a field without a
/// maximum ([`largest_stored`]), what the project database holds, its digits grouped with `separator`.
pub fn above(max: TokenBudget, separator: &str) -> Message {
  if max == largest_stored() {
    return Message::new("tokens.errors.tooLarge").with("max", grouped(max.get(), separator));
  }
  Message::new("tokens.errors.aboveMax").with("max", compact_tokens(max.get()))
}

/// A budget field's text read as a budget of at most `max`.
pub fn read_budget(text: &str, separator: &str, max: TokenBudget) -> Result<TokenBudget, Message> {
  parse_budget(text, separator, max).map_err(|error| budget_error(error, separator))
}

/// A time limit field's text: whole minutes above zero.
pub fn read_minutes(text: &str) -> Result<Duration, Message> {
  let text = text.trim();
  if text.is_empty() {
    return Err(Message::new("errors.timeLimitEmpty"));
  }
  let minutes: u64 = text.parse().map_err(|_| Message::new("errors.timeLimitInvalid"))?;
  time_limit(minutes)
}

/// A time limit of `minutes` whole minutes, above zero and at most [`MAX_STORED_MINUTES`].
pub fn time_limit(minutes: u64) -> Result<Duration, Message> {
  if minutes == 0 {
    return Err(Message::new("errors.timeLimitZero"));
  }
  if minutes > MAX_STORED_MINUTES {
    return Err(Message::new("errors.timeLimitTooLong").with("max", MAX_STORED_MINUTES.to_string()));
  }
  minutes
    .checked_mul(SECONDS_PER_MINUTE)
    .map(Duration::from_secs)
    .ok_or_else(|| Message::new("errors.timeLimitInvalid"))
}

/// An agent slots field's text: a whole number from [`AgentSlots::MIN`] to [`AgentSlots::MAX`].
pub fn read_agent_slots(text: &str) -> Result<AgentSlots, Message> {
  let count: u64 = text.trim().parse().map_err(|_| agent_slots_error())?;
  agent_slots(count)
}

/// `count` agents at once, from [`AgentSlots::MIN`] to [`AgentSlots::MAX`].
pub fn agent_slots(count: u64) -> Result<AgentSlots, Message> {
  AgentSlots::new(count).map_err(|_| agent_slots_error())
}

fn agent_slots_error() -> Message {
  Message::new("settings.limits.agentSlotsInvalid")
    .with("min", AgentSlots::MIN.to_string())
    .with("max", AgentSlots::MAX.to_string())
}

/// A time limit in a field: its whole minutes.
pub fn minutes_text(limit: Duration) -> String {
  whole_minutes(limit).to_string()
}

/// Default time limits offered by Project settings › Limits (`bPPG9`, open in `u5Vn2`), in
/// minutes of active agent time: eight choices, the menu's height without scrolling (`u0ByM`);
/// 30 is the default (`oLDmx`).
pub const TIME_LIMIT_CHOICES: [u64; 8] = [15, 30, 45, 60, 90, 120, 180, 240];

/// The minutes the time limit select offers: [`TIME_LIMIT_CHOICES`] and `current`, the stored
/// limit, when it is none of them, in ascending order.
pub fn time_limit_choices(current: Option<u64>) -> Vec<u64> {
  let mut choices = TIME_LIMIT_CHOICES.to_vec();
  if let Some(current) = current.filter(|current| *current > 0 && !choices.contains(current)) {
    let at = choices.partition_point(|choice| *choice < current);
    choices.insert(at, current);
  }
  choices
}

/// Whole minutes of `duration`, rounded down.
pub fn whole_minutes(duration: Duration) -> u64 {
  duration.as_secs() / SECONDS_PER_MINUTE
}

/// The Limits tab's fields as entered.
pub struct LimitsForm<'a> {
  pub max_run_budget: &'a str,
  pub default_run_budget: &'a str,
  pub daily_budget: &'a str,
  pub default_time_limit: &'a str,
  pub agent_slots: &'a str,
  /// The locale's group separator.
  pub separator: &'a str,
}

/// Why the Limits tab cannot be saved, per field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LimitsErrors {
  pub max_run_budget: Option<Message>,
  pub default_run_budget: Option<Message>,
  pub daily_budget: Option<Message>,
  pub default_time_limit: Option<Message>,
  pub agent_slots: Option<Message>,
}

impl LimitsForm<'_> {
  pub fn limits(&self) -> Result<ProjectLimits, Box<LimitsErrors>> {
    let budget = |text| read_budget(text, self.separator, unbounded());
    ReadLimits {
      max_run_budget: budget(self.max_run_budget),
      default_run_budget: budget(self.default_run_budget),
      daily_budget: budget(self.daily_budget),
      default_time_limit: read_minutes(self.default_time_limit),
      agent_slots: read_agent_slots(self.agent_slots),
    }
    .limits()
  }
}

/// Each limit as read, from the Limits tab's fields or from Orchester's MCP server's arguments.
pub struct ReadLimits {
  pub max_run_budget: Result<TokenBudget, Message>,
  pub default_run_budget: Result<TokenBudget, Message>,
  pub daily_budget: Result<TokenBudget, Message>,
  pub default_time_limit: Result<Duration, Message>,
  pub agent_slots: Result<AgentSlots, Message>,
}

impl ReadLimits {
  /// The limits, or why they cannot be saved: each limit's own error, else the checks between them.
  pub fn limits(self) -> Result<ProjectLimits, Box<LimitsErrors>> {
    let (Ok(max), Ok(default), Ok(daily), Ok(time), Ok(slots)) = (
      &self.max_run_budget,
      &self.default_run_budget,
      &self.daily_budget,
      &self.default_time_limit,
      &self.agent_slots,
    ) else {
      return Err(Box::new(LimitsErrors {
        max_run_budget: self.max_run_budget.err(),
        default_run_budget: self.default_run_budget.err(),
        daily_budget: self.daily_budget.err(),
        default_time_limit: self.default_time_limit.err(),
        agent_slots: self.agent_slots.err(),
      }));
    };
    let slots = *slots;
    let limits = ProjectLimits::new(*max, *default, *daily, *time).map_err(|error| {
      Box::new(match error {
        ProjectLimitsError::DefaultAboveMaximum { max, .. } => LimitsErrors {
          default_run_budget: Some(
            Message::new("tokens.errors.defaultAboveMax").with("max", compact_tokens(max.get())),
          ),
          ..LimitsErrors::default()
        },
        ProjectLimitsError::ZeroTimeLimit => LimitsErrors {
          default_time_limit: Some(Message::new("errors.timeLimitZero")),
          ..LimitsErrors::default()
        },
      })
    })?;
    Ok(limits.with_agent_slots(slots))
  }
}
