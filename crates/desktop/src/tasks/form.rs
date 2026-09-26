//! New task form (02.09 `l2YA4`) values as entered, and their mapping to `TaskDetails`.
//! Everything except the title may be left out of a draft; what is entered must be valid.

use orchester_core::{
  id::ResourceId,
  name::{Name, NameError},
  task::{LimitsError, MissingDetail, TaskDetails, TaskLimits},
  tokens::TokenBudget,
};

use crate::{
  budget::{read_budget, read_minutes},
  messages::Message,
};

const UNTITLED: &str = "untitled";

/// Field values of the new task form, as entered.
pub struct TaskForm<'a> {
  pub title: &'a str,
  pub brief: &'a str,
  /// One entry per criterion row; blank rows are not saved.
  pub criteria: &'a [String],
  pub pipeline: Option<ResourceId>,
  /// Whole tokens, grouped with `separator` or not, e.g. `1,400,000`.
  pub budget: &'a str,
  /// Whole minutes of active agent time.
  pub time_limit: &'a str,
  /// The locale's group separator.
  pub separator: &'a str,
  /// The project's maximum run budget.
  pub max_budget: TokenBudget,
}

/// Why the form cannot be saved yet, per place on the form.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormErrors {
  pub title: Option<Message>,
  pub budget: Option<Message>,
  pub time_limit: Option<Message>,
}

impl FormErrors {
  /// Why Save as draft is unavailable, shown beside it: the title error itself, and one
  /// reason for limits that are not valid (their fields say what is wrong with each).
  pub fn save_reasons(&self) -> Vec<Message> {
    let limits =
      (self.budget.is_some() || self.time_limit.is_some()).then(|| Message::new("tasks.new.limitsBlockSave"));
    self.title.iter().cloned().chain(limits).collect()
  }
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
  /// valid yet. Limits that are entered but not valid are not listed: their error already
  /// says what is wrong, and one problem gets one message.
  pub fn missing(&self) -> Vec<MissingDetail> {
    let title = Name::new(self.title).unwrap_or_else(|_| {
      // `TaskDetails::missing` never looks at the title, so any valid name stands in for it.
      Name::new(UNTITLED).expect("the stand-in title is a valid name")
    });
    let limits = self.limits();
    let invalid_limits = limits.is_err();
    TaskDetails {
      title,
      brief: self.brief.to_owned(),
      acceptance_criteria: self.criteria.to_vec(),
      pipeline: self.pipeline,
      limits: limits.ok().flatten(),
    }
    .missing()
    .into_iter()
    .filter(|detail| !(invalid_limits && *detail == MissingDetail::Limits))
    .collect()
  }

  /// Limits are a pair: both empty leaves them missing, one alone is an error on the other.
  fn limits(&self) -> Result<Option<TaskLimits>, (Option<Message>, Option<Message>)> {
    let (budget, time_limit) = (self.budget.trim(), self.time_limit.trim());
    if budget.is_empty() && time_limit.is_empty() {
      return Ok(None);
    }
    let budget = if budget.is_empty() {
      Err(Message::new("tokens.errors.empty"))
    } else {
      read_budget(budget, self.separator, self.max_budget)
    };
    let time_limit = if time_limit.is_empty() {
      Err(Message::new("errors.timeLimitRequired"))
    } else {
      read_minutes(time_limit)
    };
    let (budget, time_limit) = match (budget, time_limit) {
      (Ok(budget), Ok(time_limit)) => (budget, time_limit),
      (budget, time_limit) => return Err((budget.err(), time_limit.err())),
    };
    TaskLimits::new(budget, time_limit)
      .map(Some)
      .map_err(|LimitsError::ZeroDuration| (None, Some(Message::new("errors.timeLimitZero"))))
  }
}

fn title_error(error: &NameError) -> Message {
  match error {
    NameError::Blank => Message::new("errors.titleRequired"),
    NameError::TooLong { .. } => crate::messages::name_error(error),
  }
}
