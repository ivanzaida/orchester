//! A reviewed first-run group: ordered tasks, task concurrency and a shared token cap.

use std::{collections::HashSet, num::NonZeroU32};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  id::{IdempotencyKey, ResourceId, TaskId},
  resource::VersionNumber,
  run::RunStatus,
  tokens::{TokenBudget, TokensUsed},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scheduling {
  Parallel(NonZeroU32),
  Sequential,
}

/// Why reviewed tasks are waiting; stored independently of a run that may not yet exist.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupHold {
  SessionEnded,
  AgentCapacity,
  DailyBudget,
  GroupCap,
  PreviousTask,
  Concurrency,
  FailedTask,
  Preflight(Vec<crate::run::PreflightProblem>),
  StartFailed,
}

/// The immutable published pipeline shown for one task in the scheduling review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewedPipeline {
  pub task: TaskId,
  pub pipeline: ResourceId,
  pub version: VersionNumber,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GroupError {
  #[error("select at least one task")]
  Empty,
  #[error("a task occurs more than once")]
  Duplicate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskGroup {
  key: IdempotencyKey,
  tasks: Vec<TaskId>,
  scheduling: Scheduling,
  cap: TokenBudget,
}

impl TaskGroup {
  pub fn new(
    key: IdempotencyKey,
    tasks: Vec<TaskId>,
    scheduling: Scheduling,
    cap: TokenBudget,
  ) -> Result<Self, GroupError> {
    if tasks.is_empty() {
      return Err(GroupError::Empty);
    }
    if tasks.iter().collect::<HashSet<_>>().len() != tasks.len() {
      return Err(GroupError::Duplicate);
    }
    Ok(Self {
      key,
      tasks,
      scheduling,
      cap,
    })
  }
  pub const fn key(&self) -> IdempotencyKey {
    self.key
  }
  pub fn tasks(&self) -> &[TaskId] {
    &self.tasks
  }
  pub const fn scheduling(&self) -> Scheduling {
    self.scheduling
  }
  pub const fn cap(&self) -> TokenBudget {
    self.cap
  }

  /// Starts in reviewed order. Waiting, paused and interrupted runs hold a task slot; failure
  /// holds remaining work. Sequential advances only once the previous task is ready for review.
  pub fn eligible(&self, statuses: &[Option<RunStatus>], used: TokensUsed) -> Vec<TaskId> {
    if statuses.len() != self.tasks.len()
      || used.reached(self.cap)
      || statuses
        .iter()
        .flatten()
        .any(|s| matches!(s, RunStatus::Failed(_) | RunStatus::Cancelled))
    {
      return Vec::new();
    }
    let capacity = match self.scheduling {
      Scheduling::Parallel(n) => n.get() as usize,
      Scheduling::Sequential => 1,
    };
    let occupied = statuses
      .iter()
      .flatten()
      .filter(|s| !matches!(s, RunStatus::Succeeded))
      .count();
    let pending = self
      .tasks
      .iter()
      .zip(statuses)
      .filter(|(_, status)| status.is_none())
      .map(|(task, _)| *task);
    pending.take(capacity.saturating_sub(occupied)).collect()
  }
}
