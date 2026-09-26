//! Tasks before execution: drafts can be saved incomplete, but queueing requires every
//! detail a run depends on (design: "missing required details are surfaced before queueing").

use std::time::Duration;

use thiserror::Error;

use crate::{
  id::{ResourceId, TaskId},
  name::Name,
  project::TaskKey,
  time::Timestamp,
  tokens::TokenBudget,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LimitsError {
  #[error("run time limit must be greater than zero")]
  ZeroDuration,
}

/// Per-task token budget and time limit on active agent time; each run of the task starts with
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskLimits {
  budget: TokenBudget,
  max_duration: Duration,
}

impl TaskLimits {
  pub fn new(budget: TokenBudget, max_duration: Duration) -> Result<Self, LimitsError> {
    if max_duration.is_zero() {
      return Err(LimitsError::ZeroDuration);
    }
    Ok(Self { budget, max_duration })
  }

  pub const fn budget(self) -> TokenBudget {
    self.budget
  }

  pub const fn max_duration(self) -> Duration {
    self.max_duration
  }
}

/// User-editable content of a task. Everything but the title may be missing in a draft.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDetails {
  pub title: Name,
  pub brief: String,
  pub acceptance_criteria: Vec<String>,
  pub pipeline: Option<ResourceId>,
  pub limits: Option<TaskLimits>,
}

/// A detail a task must have before it can be queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MissingDetail {
  Brief,
  AcceptanceCriteria,
  Pipeline,
  Limits,
}

impl TaskDetails {
  pub fn missing(&self) -> Vec<MissingDetail> {
    let checks = [
      (self.brief.trim().is_empty(), MissingDetail::Brief),
      (
        self
          .acceptance_criteria
          .iter()
          .all(|criterion| criterion.trim().is_empty()),
        MissingDetail::AcceptanceCriteria,
      ),
      (self.pipeline.is_none(), MissingDetail::Pipeline),
      (self.limits.is_none(), MissingDetail::Limits),
    ];
    checks
      .into_iter()
      .filter_map(|(missing, detail)| missing.then_some(detail))
      .collect()
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskStatus {
  Draft,
  Queued,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueError {
  #[error("only draft tasks can be queued; this task is {0:?}")]
  NotDraft(TaskStatus),
  #[error("task is missing {0:?}")]
  Incomplete(Vec<MissingDetail>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
  id: TaskId,
  key: TaskKey,
  details: TaskDetails,
  status: TaskStatus,
  created_at: Timestamp,
}

impl Task {
  /// A new task always starts as a draft.
  pub const fn draft(id: TaskId, key: TaskKey, details: TaskDetails, created_at: Timestamp) -> Self {
    Self::from_stored(id, key, details, TaskStatus::Draft, created_at)
  }

  /// Rebuilds a persisted task.
  pub const fn from_stored(
    id: TaskId,
    key: TaskKey,
    details: TaskDetails,
    status: TaskStatus,
    created_at: Timestamp,
  ) -> Self {
    Self {
      id,
      key,
      details,
      status,
      created_at,
    }
  }

  pub const fn id(&self) -> TaskId {
    self.id
  }

  pub const fn key(&self) -> &TaskKey {
    &self.key
  }

  pub const fn details(&self) -> &TaskDetails {
    &self.details
  }

  pub const fn status(&self) -> TaskStatus {
    self.status
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  /// Moves a complete draft to the queue; an incomplete one reports what is missing.
  pub fn queue(&mut self) -> Result<(), QueueError> {
    if self.status != TaskStatus::Draft {
      return Err(QueueError::NotDraft(self.status));
    }
    let missing = self.details.missing();
    if !missing.is_empty() {
      return Err(QueueError::Incomplete(missing));
    }
    self.status = TaskStatus::Queued;
    Ok(())
  }
}
