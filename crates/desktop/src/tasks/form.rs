//! New task form (02.09 `l2YA4`) values as entered, and their mapping to `TaskDetails`.
//! Everything except the title may be left out of a draft; what is entered must be valid.

use std::time::Duration;

use orchester_core::{
  id::ResourceId,
  money::{Currency, Money},
  name::{Name, NameError},
  task::{LimitsError, MissingDetail, TaskDetails, TaskLimits},
};

use crate::messages::Message;

/// Currency of task budgets. Projects have no currency setting yet, and the design states
/// every budget in US dollars.
pub const BUDGET_CURRENCY: Currency = Currency::USD;

const SECONDS_PER_MINUTE: u64 = 60;
const UNTITLED: &str = "untitled";

/// Field values of the new task form, as entered.
pub struct TaskForm<'a> {
  pub title: &'a str,
  pub brief: &'a str,
  /// One entry per criterion row; blank rows are not saved.
  pub criteria: &'a [String],
  pub pipeline: Option<ResourceId>,
  /// Amount in [`BUDGET_CURRENCY`], e.g. `5` or `5.50`.
  pub budget: &'a str,
  /// Whole minutes.
  pub time_limit: &'a str,
}

/// Why the form cannot be saved yet, per place on the form.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormErrors {
  pub title: Option<Message>,
  pub budget: Option<Message>,
  pub time_limit: Option<Message>,
}

impl TaskForm<'_> {
  pub fn details(&self) -> Result<TaskDetails, FormErrors> {
    let title = Name::new(self.title).map_err(|error| title_error(&error));
    let limits = self.limits();
    match (title, limits) {
      (Ok(title), Ok(limits)) => Ok(TaskDetails {
        title,
        brief: self.brief.trim().to_owned(),
        acceptance_criteria: self
          .criteria
          .iter()
          .map(|criterion| criterion.trim())
          .filter(|criterion| !criterion.is_empty())
          .map(str::to_owned)
          .collect(),
        pipeline: self.pipeline,
        limits,
      }),
      (title, limits) => {
        let (budget, time_limit) = limits.err().unwrap_or_default();
        Err(FormErrors {
          title: title.err(),
          budget,
          time_limit,
        })
      }
    }
  }

  /// Details the task still needs before it can be queued, also while the title is not
  /// valid yet. Limits that are not valid count as missing; their error is shown as well.
  pub fn missing(&self) -> Vec<MissingDetail> {
    let title = Name::new(self.title).unwrap_or_else(|_| {
      // `TaskDetails::missing` never looks at the title, so any valid name stands in for it.
      Name::new(UNTITLED).expect("the stand-in title is a valid name")
    });
    TaskDetails {
      title,
      brief: self.brief.to_owned(),
      acceptance_criteria: self.criteria.to_vec(),
      pipeline: self.pipeline,
      // Invalid limits are missing for queueing; `details` reports why they are invalid.
      limits: self.limits().ok().flatten(),
    }
    .missing()
  }

  /// Limits are a pair: both empty leaves them missing, one alone is an error on the other.
  fn limits(&self) -> Result<Option<TaskLimits>, (Option<Message>, Option<Message>)> {
    let (budget, time_limit) = (self.budget.trim(), self.time_limit.trim());
    if budget.is_empty() && time_limit.is_empty() {
      return Ok(None);
    }
    let budget = if budget.is_empty() {
      Err(Message::new("errors.budgetRequired"))
    } else {
      Money::parse(budget, BUDGET_CURRENCY).map_err(|_| Message::new("errors.budgetInvalid"))
    };
    let time_limit = if time_limit.is_empty() {
      Err(Message::new("errors.timeLimitRequired"))
    } else {
      time_limit
        .parse::<u64>()
        .ok()
        .and_then(|minutes| minutes.checked_mul(SECONDS_PER_MINUTE))
        .map(Duration::from_secs)
        .ok_or_else(|| Message::new("errors.timeLimitInvalid"))
    };
    let (budget, time_limit) = match (budget, time_limit) {
      (Ok(budget), Ok(time_limit)) => (budget, time_limit),
      (budget, time_limit) => return Err((budget.err(), time_limit.err())),
    };
    TaskLimits::new(budget, time_limit)
      .map(Some)
      .map_err(|error| match error {
        LimitsError::NonPositiveBudget => (Some(Message::new("errors.budgetNotPositive")), None),
        LimitsError::ZeroDuration => (None, Some(Message::new("errors.timeLimitZero"))),
      })
  }
}

fn title_error(error: &NameError) -> Message {
  match error {
    NameError::Blank => Message::new("errors.titleRequired"),
    NameError::TooLong { .. } => crate::messages::name_error(error),
  }
}
