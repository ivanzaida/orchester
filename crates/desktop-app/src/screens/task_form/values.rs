//! What the form's fields hold, as typed: read into the workspace's [`TaskForm`] for its checks, compared
//! with what is stored to know whether something changed, and kept on this computer while it is not
//! saved (`kept`).

use orchester_core::{
  id::ResourceId,
  resource::Deliverable,
  task::{
    Task, TaskDetails,
    estimate::{EstimateSource, StoryPoints},
    organization::{Priority, TaskOrganization},
  },
  tokens::grouped,
};
use orchester_workspace::{budget::minutes_text, tasks::form::TaskForm};

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
  /// The person's story points; `None`: Automatic, Orchester's estimate (ORC-49).
  pub points: Option<StoryPoints>,
  /// *Don't merge automatically* (ORC-69), a part of the task's organization.
  pub no_automatic_merge: bool,
  /// The task's priority, a part of its organization; no priority by default.
  pub priority: Priority,
  /// What the task's runs deliver (ORC-93), a part of its organization; `None`: as its pipeline says.
  pub delivers: Option<Deliverable>,
}

impl lurq::app::component::DevtoolsInspectable for Values {}

impl Values {
  /// The fields of stored details, its budget grouped with `separator`, without points of a person, with *Don't merge
  /// automatically* off, no priority and what the pipeline delivers.
  pub fn of_details(details: &TaskDetails, separator: &str) -> Self {
    Self {
      title: details.title.as_str().to_owned(),
      brief: details.brief.clone(),
      criteria: details.acceptance_criteria.clone(),
      pipeline: details.pipeline,
      budget: details
        .limits
        .budget()
        .map(|budget| grouped(budget.get(), separator))
        .unwrap_or_default(),
      time_limit: minutes_text(details.limits.max_duration()),
      ..Self::default()
    }
  }

  /// The fields of a stored task, with the story points a person set, if any, and its organization's *Don't merge
  /// automatically*, priority and what its runs deliver.
  pub fn of_task(task: &Task, separator: &str) -> Self {
    let organization = task.organization();
    Self {
      points: person_points(task),
      no_automatic_merge: organization.no_automatic_merge(),
      priority: organization.priority(),
      delivers: organization.delivers(),
      ..Self::of_details(task.details(), separator)
    }
  }

  /// The organization a save stores, given the one the edits were made on (`base`, as these values were filled from
  /// `base_values`) and the one stored now (`current`): *Don't merge automatically*, the priority and what the runs
  /// deliver as the person set them, or as they are stored now where the person left them as they were.
  pub fn organization(&self, base_values: &Self, current: &TaskOrganization) -> TaskOrganization {
    let flag = if self.no_automatic_merge == base_values.no_automatic_merge {
      current.no_automatic_merge()
    } else {
      self.no_automatic_merge
    };
    let priority = if self.priority == base_values.priority {
      current.priority()
    } else {
      self.priority
    };
    let delivers = if self.delivers == base_values.delivers {
      current.delivers()
    } else {
      self.delivers
    };
    current
      .clone()
      .with_no_automatic_merge(flag)
      .with_priority(priority)
      .with_delivers(delivers)
  }

  /// The values as the workspace checks them. Limits are optional and have no maximum (the person's
  /// decision of 2026-10-09): only text that is not a budget or whole minutes is refused.
  pub fn task_form<'a>(&'a self, separator: &'a str) -> TaskForm<'a> {
    TaskForm {
      title: &self.title,
      brief: &self.brief,
      criteria: &self.criteria,
      pipeline: self.pipeline,
      budget: &self.budget,
      time_limit: &self.time_limit,
      separator,
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
      points: self.points,
      no_automatic_merge: self.no_automatic_merge,
      priority: self.priority,
      delivers: self.delivers,
    }
  }

  /// Whether the values differ from `stored` in anything a save would store.
  pub fn differ_from(&self, stored: &Self, separator: &str) -> bool {
    self.normalized(separator) != stored.normalized(separator)
  }
}

/// The story points a person set for `task`; `None` when it has none or only Orchester's.
pub fn person_points(task: &Task) -> Option<StoryPoints> {
  task
    .estimate()
    .filter(|estimate| estimate.source() == EstimateSource::Person)
    .map(|estimate| estimate.points())
}
