//! Token budgets and time limits as people enter and read them (`docs/design/orchester-workspace.md`,
//! "Token budgets", "Stopped at limit"): field errors, the Project settings › Limits form and
//! whole-minute time limits of active agent time.

use std::time::Duration;

use orchester_core::{
  limits::{ProjectLimits, ProjectLimitsError},
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

/// What a person reads for a budget field's text that is not a budget.
pub fn budget_error(error: BudgetInputError, separator: &str) -> Message {
  match error {
    BudgetInputError::Empty => Message::new("tokens.errors.empty"),
    BudgetInputError::NotDigits => {
      Message::new("tokens.errors.digits").with("example", grouped(EXAMPLE_BUDGET, separator))
    }
    BudgetInputError::Zero => Message::new("tokens.errors.zero"),
    BudgetInputError::AboveMaximum { max } => {
      Message::new("tokens.errors.aboveMax").with("max", compact_tokens(max.get()))
    }
  }
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
  if minutes == 0 {
    return Err(Message::new("errors.timeLimitZero"));
  }
  minutes
    .checked_mul(SECONDS_PER_MINUTE)
    .map(Duration::from_secs)
    .ok_or_else(|| Message::new("errors.timeLimitInvalid"))
}

/// A time limit in a field: its whole minutes.
pub fn minutes_text(limit: Duration) -> String {
  whole_minutes(limit).to_string()
}

/// Default time limits offered by Project settings › Limits (`bPPG9`), in minutes of active
/// agent time. The design draws the select closed with 30 minutes, the default; it has no open
/// menu, so the choices are ours: up to 8, the menu's height without scrolling (`u0ByM`).
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
}

impl LimitsForm<'_> {
  pub fn limits(&self) -> Result<ProjectLimits, Box<LimitsErrors>> {
    let budget = |text| read_budget(text, self.separator, unbounded());
    let max = budget(self.max_run_budget);
    let default = budget(self.default_run_budget);
    let daily = budget(self.daily_budget);
    let time = read_minutes(self.default_time_limit);
    let (Ok(max), Ok(default), Ok(daily), Ok(time)) = (&max, &default, &daily, &time) else {
      return Err(Box::new(LimitsErrors {
        max_run_budget: max.err(),
        default_run_budget: default.err(),
        daily_budget: daily.err(),
        default_time_limit: time.err(),
      }));
    };
    ProjectLimits::new(*max, *default, *daily, *time).map_err(|error| {
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
    })
  }
}
