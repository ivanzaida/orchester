//! What the form's fields hold, as typed: read into the workspace's [`TaskForm`] for its checks, compared
//! with what is stored to know whether something changed, and kept on this computer while it is not
//! saved (`kept`).

use orchester_core::{id::ResourceId, task::TaskDetails, tokens::grouped};
use orchester_workspace::{
  budget::{minutes_text, unbounded},
  tasks::form::TaskForm,
};

/// The fields' values. Criteria are one entry per row, blank rows included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Values {
  pub title: String,
  pub brief: String,
  pub criteria: Vec<String>,
  pub pipeline: Option<ResourceId>,
  /// Whole tokens as typed, grouped or not; empty for no limit.
  pub budget: String,
  /// Whole minutes as typed; empty for no limit.
  pub time_limit: String,
}

impl lurq::app::component::DevtoolsInspectable for Values {}

impl Values {
  /// The fields of a stored task, its budget grouped with `separator`.
  pub fn of_details(details: &TaskDetails, separator: &str) -> Self {
    Self {
      title: details.title.as_str().to_owned(),
      brief: details.brief.clone(),
      criteria: details.acceptance_criteria.clone(),
      pipeline: details.pipeline,
      budget: details
        .limits
        .map(|limits| grouped(limits.budget().get(), separator))
        .unwrap_or_default(),
      time_limit: details
        .limits
        .map(|limits| minutes_text(limits.max_duration()))
        .unwrap_or_default(),
    }
  }

  /// The values as the workspace checks them. Run budgets have no maximum (the person's decision of
  /// 2026-10-09): only text that is not a budget is refused.
  pub fn task_form<'a>(&'a self, separator: &'a str) -> TaskForm<'a> {
    TaskForm {
      title: &self.title,
      brief: &self.brief,
      criteria: &self.criteria,
      pipeline: self.pipeline,
      budget: &self.budget,
      time_limit: &self.time_limit,
      separator,
      max_budget: unbounded(),
    }
  }

  /// What a save would store of the values: spaces at the ends and blank criteria do not count, nor
  /// how a budget's digits are grouped.
  pub fn normalized(&self, separator: &str) -> Self {
    let digits = |text: &str| text.trim().replace(separator, "");
    Self {
      title: self.title.trim().to_owned(),
      brief: self.brief.trim().to_owned(),
      criteria: self
        .criteria
        .iter()
        .map(|criterion| criterion.trim())
        .filter(|criterion| !criterion.is_empty())
        .map(str::to_owned)
        .collect(),
      pipeline: self.pipeline,
      budget: digits(&self.budget),
      time_limit: self.time_limit.trim().to_owned(),
    }
  }

  /// Whether the values differ from `stored` in anything a save would store.
  pub fn differ_from(&self, stored: &Self, separator: &str) -> bool {
    self.normalized(separator) != stored.normalized(separator)
  }
}
