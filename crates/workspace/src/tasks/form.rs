//! New task form (02.09 `l2YA4`) values as entered, and their mapping to `TaskDetails`.
//! Everything except the title may be left out of a draft; what is entered must be valid.

use orchester_core::{
  id::ResourceId,
  name::{Name, NameError},
  task::{LimitsError, MissingDetail, TaskDetails, TaskLimits},
};

use crate::{
  budget::{read_minutes, read_optional_budget},
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
  /// Whole tokens, grouped with `separator` or not, e.g. `1,400,000`; empty for no budget.
  pub budget: &'a str,
  /// Whole minutes of active agent time; empty for no time limit.
  pub time_limit: &'a str,
  /// The locale's group separator.
  pub separator: &'a str,
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
  /// valid yet. Limits are never missing: empty ones are no limit.
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
      limits: TaskLimits::NONE,
    }
    .missing()
  }

  /// The budget and the time limit, each optional: an empty field is no limit.
  fn limits(&self) -> Result<TaskLimits, (Option<Message>, Option<Message>)> {
    let budget = read_optional_budget(self.budget, self.separator);
    let time_limit = read_minutes(self.time_limit);
    let (budget, time_limit) = match (budget, time_limit) {
      (Ok(budget), Ok(time_limit)) => (budget, time_limit),
      (budget, time_limit) => return Err((budget.err(), time_limit.err())),
    };
    TaskLimits::new(budget, time_limit).map_err(|error| {
      let message = match error {
        LimitsError::ZeroDuration => Message::new("errors.timeLimitZero"),
        LimitsError::TooLong => {
          Message::new("errors.timeLimitTooLong").with("max", crate::budget::MAX_STORED_MINUTES.to_string())
        }
      };
      (None, Some(message))
    })
  }
}

fn title_error(error: &NameError) -> Message {
  match error {
    NameError::Blank => Message::new("errors.titleRequired"),
    NameError::TooLong { .. } => crate::messages::name_error(error),
  }
}
