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
  /// Every unstarted task of the group waits for linked blocking tasks, `by`, to finish. Ends on
  /// its own when they do.
  Blocked {
    by: Vec<TaskId>,
  },
}

impl GroupHold {
  /// Whether the hold ends without a person: an agent slot or the daily budget frees up, a
  /// running task of the group settles, or the blocking tasks finish. The others wait for someone
  /// to act (a failed task, the spent cap, a start the person must fix, a reopened project).
  pub const fn clears_on_its_own(&self) -> bool {
    matches!(
      self,
      Self::AgentCapacity | Self::DailyBudget | Self::PreviousTask | Self::Concurrency | Self::Blocked { .. }
    )
  }

  /// Whether a person may take the group's unstarted tasks back to the drafts: the hold does not
  /// end on its own, and is not the reopened-project hold, which Start run releases instead; or
  /// the tasks wait for blocking tasks, which may never finish.
  pub const fn releases_unstarted(&self) -> bool {
    matches!(self, Self::Blocked { .. }) || (!self.clears_on_its_own() && !matches!(self, Self::SessionEnded))
  }

  /// Whether the hold stays when the project is reopened: a failed task or the spent cap holds
  /// the group whatever the session. Every other hold of unstarted tasks becomes the session hold,
  /// so they start again only through an explicit Start run (a failed start included).
  pub const fn outlives_session(&self) -> bool {
    matches!(self, Self::FailedTask | Self::GroupCap)
  }
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

  /// Why a task of the group that cannot start now waits: the spent cap, a failed or cancelled
  /// task, the blocking tasks every unstarted task waits for, or otherwise its turn (the previous
  /// task, or the concurrency limit). `blockers` are each task's unfinished blocking tasks, in the
  /// group's order; a missing entry is a task that is not blocked.
  pub fn waiting_reason(
    &self,
    statuses: &[Option<RunStatus>],
    used: TokensUsed,
    blockers: &[Vec<TaskId>],
  ) -> GroupHold {
    let statuses = &waiting_view(statuses, blockers);
    if used.reached(self.cap) {
      return GroupHold::GroupCap;
    }
    if stopped(statuses) {
      return GroupHold::FailedTask;
    }
    if let Some(by) = all_blocked(statuses, blockers) {
      return GroupHold::Blocked { by };
    }
    match self.scheduling {
      Scheduling::Sequential => GroupHold::PreviousTask,
      Scheduling::Parallel(_) => GroupHold::Concurrency,
    }
  }

  /// Why the group holds its work, if it does: the spent cap or a failed task hold it whatever is
  /// left; otherwise it holds only while tasks are unstarted and none of them can start.
  pub fn hold(&self, statuses: &[Option<RunStatus>], used: TokensUsed, blockers: &[Vec<TaskId>]) -> Option<GroupHold> {
    let statuses = &waiting_view(statuses, blockers);
    let blocked = used.reached(self.cap) || stopped(statuses);
    let waiting = statuses.iter().any(Option::is_none) && self.eligible(statuses, used, blockers).is_empty();
    (blocked || waiting).then(|| self.waiting_reason(statuses, used, blockers))
  }

  /// Whether the group has work that goes on without a person: an unstarted task that can start
  /// now, or one waiting for its turn behind a member that is starting or running. A member that is
  /// paused, stopped at a limit or interrupted only moves again when a person continues it, so a
  /// turn behind it waits for that person too, as do the spent cap and a failed task. Tasks waiting
  /// for blocking tasks outside the group are not the group's own work: whoever runs the blocking
  /// tasks decides when they finish.
  pub fn has_work_of_its_own(
    &self,
    statuses: &[Option<RunStatus>],
    used: TokensUsed,
    blockers: &[Vec<TaskId>],
  ) -> bool {
    let statuses = &waiting_view(statuses, blockers);
    if !statuses.iter().any(Option::is_none) {
      return false;
    }
    match self.hold(statuses, used, blockers) {
      None => true,
      Some(GroupHold::PreviousTask | GroupHold::Concurrency) => statuses
        .iter()
        .flatten()
        .any(|status| matches!(status, RunStatus::Pending | RunStatus::Running)),
      Some(_) => false,
    }
  }

  /// Every task of the group has a first run and every one of those runs has ended.
  pub fn finished(statuses: &[Option<RunStatus>]) -> bool {
    statuses
      .iter()
      .all(|status| status.as_ref().is_some_and(RunStatus::is_terminal))
  }

  /// Starts in reviewed order. Waiting, paused and interrupted runs hold a task slot; failure
  /// holds remaining work. Sequential advances only once the previous task is ready for review. A
  /// blocked task is passed over, so the tasks after it start in its place; it starts once its
  /// blocking tasks finish, even when one of them is a later task of the same group.
  pub fn eligible(&self, statuses: &[Option<RunStatus>], used: TokensUsed, blockers: &[Vec<TaskId>]) -> Vec<TaskId> {
    let statuses = &waiting_view(statuses, blockers);
    if statuses.len() != self.tasks.len() || used.reached(self.cap) || stopped(statuses) {
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
      .enumerate()
      .filter(|(index, (_, status))| status.is_none() && !is_blocked(blockers, *index))
      .map(|(_, (task, _))| *task);
    pending.take(capacity.saturating_sub(occupied)).collect()
  }
}

/// `statuses` with a blocked task's Pending first run read as not started: it neither holds a slot
/// nor counts as work under way until its blocking tasks finish.
fn waiting_view(statuses: &[Option<RunStatus>], blockers: &[Vec<TaskId>]) -> Vec<Option<RunStatus>> {
  statuses
    .iter()
    .enumerate()
    .map(|(index, status)| match status {
      Some(RunStatus::Pending) if is_blocked(blockers, index) => None,
      other => other.clone(),
    })
    .collect()
}

fn is_blocked(blockers: &[Vec<TaskId>], index: usize) -> bool {
  blockers.get(index).is_some_and(|by| !by.is_empty())
}

/// The blocking tasks of the unstarted tasks when every one of them is blocked, in order and
/// once each; `None` while any unstarted task is free to start in its turn.
fn all_blocked(statuses: &[Option<RunStatus>], blockers: &[Vec<TaskId>]) -> Option<Vec<TaskId>> {
  let mut by = Vec::new();
  for (index, _) in statuses.iter().enumerate().filter(|(_, status)| status.is_none()) {
    let blocking = blockers.get(index).filter(|blocking| !blocking.is_empty())?;
    for task in blocking {
      if !by.contains(task) {
        by.push(*task);
      }
    }
  }
  (!by.is_empty()).then_some(by)
}

/// A failed or cancelled first run holds the rest of the group.
fn stopped(statuses: &[Option<RunStatus>]) -> bool {
  statuses
    .iter()
    .flatten()
    .any(|status| matches!(status, RunStatus::Failed(_) | RunStatus::Cancelled))
}
