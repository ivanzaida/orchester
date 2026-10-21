//! Modules: a scope of a project's tasks followed over time, such as an epic, a milestone or a sprint. A module has
//! a name, an optional description, optional start and target dates (whole local days) and is open or closed; a
//! task belongs to at most one module, and every move of a task into or out of a module is kept
//! ([`MembershipChange`]), so its burndown can show how its scope changed.
//!
//! A module is not a reviewed task group ([`crate::task_group::TaskGroup`]): a group is how a batch of tasks is
//! scheduled for its first runs (order, concurrency, a shared token cap, holds) and ends once they ran, while a
//! module is what a person plans and follows, whatever runs its tasks and however often. Neither is it a tag: a task
//! carries many tags and a tag has no dates, state or history of its own. Dependencies order tasks; a module only
//! groups them, so a blocked task stays in its module.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  id::{ModuleId, TaskId},
  name::Name,
  task::organization::ChangeActor,
  time::Timestamp,
};

/// Whether a module is still worked on; a closed one keeps its tasks and its history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleState {
  #[default]
  Open,
  Closed,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModuleError {
  #[error("a module's target date cannot be before its start date")]
  TargetBeforeStart,
  #[error("another module has this name")]
  DuplicateName,
  #[error("module {0} does not exist")]
  NotFound(ModuleId),
  #[error("the module was changed since it was read")]
  ChangedMeanwhile,
}

/// What a person sets of a module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleDetails {
  name: Name,
  description: String,
  start: Option<NaiveDate>,
  target: Option<NaiveDate>,
  state: ModuleState,
}

impl ModuleDetails {
  /// The details, the description trimmed; a target before the start is refused.
  pub fn new(
    name: Name,
    description: &str,
    (start, target): (Option<NaiveDate>, Option<NaiveDate>),
    state: ModuleState,
  ) -> Result<Self, ModuleError> {
    if let (Some(start), Some(target)) = (start, target)
      && target < start
    {
      return Err(ModuleError::TargetBeforeStart);
    }
    Ok(Self {
      name,
      description: description.trim().to_owned(),
      start,
      target,
      state,
    })
  }

  pub const fn name(&self) -> &Name {
    &self.name
  }

  pub fn description(&self) -> &str {
    &self.description
  }

  /// The first day the module counts from; without one it counts from the day it got its first task.
  pub const fn start(&self) -> Option<NaiveDate> {
    self.start
  }

  /// The day its work should be done by; without one its burndown has no ideal line.
  pub const fn target(&self) -> Option<NaiveDate> {
    self.target
  }

  pub const fn state(&self) -> ModuleState {
    self.state
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
  id: ModuleId,
  details: ModuleDetails,
  created_at: Timestamp,
}

impl Module {
  pub const fn new(id: ModuleId, details: ModuleDetails, created_at: Timestamp) -> Self {
    Self {
      id,
      details,
      created_at,
    }
  }

  pub const fn id(&self) -> ModuleId {
    self.id
  }

  pub const fn details(&self) -> &ModuleDetails {
    &self.details
  }

  pub const fn name(&self) -> &Name {
    self.details.name()
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  /// The module with `details` in place of its own.
  #[must_use]
  pub fn with_details(self, details: ModuleDetails) -> Self {
    Self { details, ..self }
  }
}

/// A task moved into a module, out of one, or from one to another, by a person or by Orchester's tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MembershipChange {
  pub task: TaskId,
  pub from: Option<ModuleId>,
  pub to: Option<ModuleId>,
  pub actor: ChangeActor,
  pub at: Timestamp,
}
