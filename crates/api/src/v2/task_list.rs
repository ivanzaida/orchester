//! The task list as an S10 read (the desktop's Tasks list, `orchester_workspace::tasks::browse`): its filters, its
//! search, its sorts and its board counts. A request leaves out what it does not narrow by; an unknown field is
//! refused.

use orchester_core::{
  id::{ModuleId, ReleaseId, TagId},
  task::organization::{Priority, WorkflowStatus},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{EventCursor, PageRequest};
use crate::wire;

/// The longest search, in characters.
pub const MAX_SEARCH_CHARS: usize = 200;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SearchTextError {
  #[error("a search has text")]
  Empty,
  #[error("a search is at most {MAX_SEARCH_CHARS} characters")]
  TooLong,
}

/// What the list's search looks for, trimmed: tasks whose title, key, former key, id or brief hold it, ignoring case.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SearchText(String);

impl SearchText {
  pub fn new(text: &str) -> Result<Self, SearchTextError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(SearchTextError::Empty);
    }
    if text.chars().count() > MAX_SEARCH_CHARS {
      return Err(SearchTextError::TooLong);
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for SearchText {
  type Error = SearchTextError;

  fn try_from(text: String) -> Result<Self, Self::Error> {
    Self::new(&text)
  }
}

impl From<SearchText> for String {
  fn from(text: SearchText) -> Self {
    text.0
  }
}

/// What narrows the task list. Each facet keeps the tasks that have any of its values; one without values keeps every
/// task. `null` among `pipelines`, `modules` or `releases` is a task without one; `all_tags` keeps only the tasks with
/// every tag of `tags`. `pipelines` are pipeline slugs, as a shared task names its pipeline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskFilters {
  #[serde(default, with = "statuses")]
  pub statuses: Vec<WorkflowStatus>,
  #[serde(default, with = "priorities")]
  pub priorities: Vec<Priority>,
  #[serde(default)]
  pub tags: Vec<TagId>,
  #[serde(default)]
  pub all_tags: bool,
  #[serde(default)]
  pub pipelines: Vec<Option<String>>,
  #[serde(default)]
  pub modules: Vec<Option<ModuleId>>,
  #[serde(default)]
  pub releases: Vec<Option<ReleaseId>>,
  #[serde(default)]
  pub search: Option<SearchText>,
}

/// The order of the list. Every order ends on the task's creation sequence, then its id, so a page boundary is stable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskSort {
  /// Oldest first: the list's own order.
  #[default]
  Created,
  Newest,
  /// By title, ignoring case.
  Title,
  /// In workflow order, the most recently changed first within each status.
  Status,
  /// Urgent first, the most recently changed first within each priority.
  Priority,
}

/// `{filters, sort, page}`: one page of the tasks `filters` keep, in `sort` order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskListRequest {
  #[serde(default)]
  pub filters: TaskFilters,
  #[serde(default)]
  pub sort: TaskSort,
  #[serde(default)]
  pub page: PageRequest,
}

/// How many of the tasks a request's [`TaskFilters`] keep are in each workflow status, at snapshot `cursor`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskBoardCounts {
  pub backlog: u32,
  pub todo: u32,
  pub in_progress: u32,
  pub in_review: u32,
  pub done: u32,
  pub canceled: u32,
  pub cursor: EventCursor,
}

mod statuses {
  use super::*;

  #[derive(Serialize, Deserialize)]
  struct Value(#[serde(with = "wire::Workflow")] WorkflowStatus);

  pub fn serialize<S: serde::Serializer>(values: &[WorkflowStatus], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(values.iter().map(|value| Value(*value)))
  }

  pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<WorkflowStatus>, D::Error> {
    Ok(
      Vec::<Value>::deserialize(deserializer)?
        .into_iter()
        .map(|value| value.0)
        .collect(),
    )
  }
}

mod priorities {
  use super::*;

  #[derive(Serialize, Deserialize)]
  struct Value(#[serde(with = "wire::PriorityWire")] Priority);

  pub fn serialize<S: serde::Serializer>(values: &[Priority], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(values.iter().map(|value| Value(*value)))
  }

  pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<Priority>, D::Error> {
    Ok(
      Vec::<Value>::deserialize(deserializer)?
        .into_iter()
        .map(|value| value.0)
        .collect(),
    )
  }
}
