//! Token budgets and time limits as people enter and read them (`docs/design/orchester-workspace.md`,
//! "Token budgets", "Stopped at limit"): field errors, the Project settings › Limits form,
//! whole-minute time limits of active agent time, how many agents work at once and how many QA apps
//! are open at once. A run budget and a time limit are optional with no maximum: an empty field is no
//! limit (user decision 2026-10-09).

use std::time::Duration;

use orchester_core::{
  limits::{AgentSlots, LONGEST_TIME_LIMIT, ProjectLimits, ProjectLimitsError, QaSlots},
  tokens::{BudgetInputError, TokenBudget, grouped, parse_budget, parse_optional_budget},
};

use crate::messages::Message;

const SECONDS_PER_MINUTE: u64 = 60;
/// The example in "Use digits only, for example 1,400,000.", grouped for the locale.
const EXAMPLE_BUDGET: u64 = 1_400_000;

/// What a person reads for a budget field's text that is not a budget.
pub fn budget_error(error: BudgetInputError, separator: &str) -> Message {
  match error {
    BudgetInputError::Empty => Message::new("tokens.errors.empty"),
    BudgetInputError::NotDigits => {
      Message::new("tokens.errors.digits").with("example", grouped(EXAMPLE_BUDGET, separator))
    }
    BudgetInputError::Zero => Message::new("tokens.errors.zero"),
    BudgetInputError::TooLarge => {
      Message::new("tokens.errors.tooLarge").with("max", grouped(TokenBudget::LARGEST, separator))
    }
  }
}

/// A budget field's text read as a budget that must be entered (the daily project budget, a task
/// group's cap).
pub fn read_budget(text: &str, separator: &str) -> Result<TokenBudget, Message> {
  parse_budget(text, separator).map_err(|error| budget_error(error, separator))
}

/// A limit's budget field: empty is no limit (`None`), anything else must be a budget.
pub fn read_optional_budget(text: &str, separator: &str) -> Result<Option<TokenBudget>, Message> {
  parse_optional_budget(text, separator).map_err(|error| budget_error(error, separator))
}

/// A time limit field's text: empty is no limit (`None`), anything else whole minutes above zero.
pub fn read_minutes(text: &str) -> Result<Option<Duration>, Message> {
  let text = text.trim();
  if text.is_empty() {
    return Ok(None);
  }
  let minutes: u64 = text.parse().map_err(|_| {
    // Digits only, but more than any number of minutes: too long rather than not minutes.
    if text.bytes().all(|byte| byte.is_ascii_digit()) {
      too_long()
    } else {
      Message::new("errors.timeLimitInvalid")
    }
  })?;
  time_limit(minutes).map(Some)
}

/// The longest time limit, [`LONGEST_TIME_LIMIT`], in whole minutes.
pub const MAX_STORED_MINUTES: u64 = LONGEST_TIME_LIMIT.as_secs() / SECONDS_PER_MINUTE;

fn too_long() -> Message {
  Message::new("errors.timeLimitTooLong").with("max", MAX_STORED_MINUTES.to_string())
}

/// A time limit of `minutes` whole minutes, above zero and at most [`MAX_STORED_MINUTES`].
pub fn time_limit(minutes: u64) -> Result<Duration, Message> {
  if minutes == 0 {
    return Err(Message::new("errors.timeLimitZero"));
  }
  if minutes > MAX_STORED_MINUTES {
    return Err(too_long());
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

/// A QA slots field's text: a whole number from [`QaSlots::MIN`] to [`QaSlots::MAX`].
pub fn read_qa_slots(text: &str) -> Result<QaSlots, Message> {
  let count: u64 = text.trim().parse().map_err(|_| qa_slots_error())?;
  qa_slots(count)
}

/// `count` QA apps at once, from [`QaSlots::MIN`] to [`QaSlots::MAX`].
pub fn qa_slots(count: u64) -> Result<QaSlots, Message> {
  QaSlots::new(count).map_err(|_| qa_slots_error())
}

fn qa_slots_error() -> Message {
  Message::new("settings.limits.qaSlotsInvalid")
    .with("min", QaSlots::MIN.to_string())
    .with("max", QaSlots::MAX.to_string())
}

/// A time limit in a field: its whole minutes, empty for no limit.
pub fn minutes_text(limit: Option<Duration>) -> String {
  limit.map(|limit| whole_minutes(limit).to_string()).unwrap_or_default()
}

/// Default time limits offered by Project settings › Limits (`bPPG9`, open in `u5Vn2`), in
/// minutes of active agent time: eight choices, the menu's height without scrolling (`u0ByM`).
/// The select offers *No limit* besides them, the default (user decision 2026-10-09).
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

/// The deprecated app's Limits tab fields as entered. An empty default run budget or time limit means new tasks
/// have none. It has no *QA apps at once* field: its limits carry the default QA slots. The new app reads its
/// fields with `crate::project_settings::LimitsText`.
pub struct LimitsForm<'a> {
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
  pub default_run_budget: Option<Message>,
  pub daily_budget: Option<Message>,
  pub default_time_limit: Option<Message>,
  pub agent_slots: Option<Message>,
  pub qa_slots: Option<Message>,
}

impl LimitsForm<'_> {
  pub fn limits(&self) -> Result<ProjectLimits, Box<LimitsErrors>> {
    ReadLimits {
      default_run_budget: read_optional_budget(self.default_run_budget, self.separator),
      daily_budget: read_budget(self.daily_budget, self.separator),
      default_time_limit: read_minutes(self.default_time_limit),
      agent_slots: read_agent_slots(self.agent_slots),
      qa_slots: Ok(QaSlots::DEFAULT),
    }
    .limits()
  }
}

/// Each limit as read, from the Limits tab's fields or from Orchester's MCP server's arguments;
/// `None` is no limit.
pub struct ReadLimits {
  pub default_run_budget: Result<Option<TokenBudget>, Message>,
  pub daily_budget: Result<TokenBudget, Message>,
  pub default_time_limit: Result<Option<Duration>, Message>,
  pub agent_slots: Result<AgentSlots, Message>,
  pub qa_slots: Result<QaSlots, Message>,
}

impl ReadLimits {
  /// The limits, or why they cannot be saved: each limit's own error.
  pub fn limits(self) -> Result<ProjectLimits, Box<LimitsErrors>> {
    let (Ok(default), Ok(daily), Ok(time), Ok(slots), Ok(qa)) = (
      &self.default_run_budget,
      &self.daily_budget,
      &self.default_time_limit,
      &self.agent_slots,
      &self.qa_slots,
    ) else {
      return Err(Box::new(LimitsErrors {
        default_run_budget: self.default_run_budget.err(),
        daily_budget: self.daily_budget.err(),
        default_time_limit: self.default_time_limit.err(),
        agent_slots: self.agent_slots.err(),
        qa_slots: self.qa_slots.err(),
      }));
    };
    let limits = ProjectLimits::new(*default, *daily, *time).map_err(|error| {
      let message = match error {
        ProjectLimitsError::ZeroTimeLimit => Message::new("errors.timeLimitZero"),
        ProjectLimitsError::TimeLimitTooLong => too_long(),
      };
      Box::new(LimitsErrors {
        default_time_limit: Some(message),
        ..LimitsErrors::default()
      })
    })?;
    Ok(limits.with_agent_slots(*slots).with_qa_slots(*qa))
  }
}
