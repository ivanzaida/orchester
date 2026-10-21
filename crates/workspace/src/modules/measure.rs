//! What a module's work is counted in: the one place the unit is chosen, for the list's progress, the burndown and
//! the tools. Tasks have no story points yet, so work is counted in tasks; ORC-49's story points plug in here, and a
//! module whose tasks have them is then counted in points.

use std::collections::HashMap;

use orchester_core::{id::TaskId, task::Task};

/// The unit a module's work is counted in, which its chart names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkUnit {
  /// Each task counts as one.
  Tasks,
}

impl WorkUnit {
  /// The unit as the tools name it.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Tasks => "tasks",
    }
  }
}

/// The unit of a module's `tasks` and the work each one is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measure {
  pub unit: WorkUnit,
  weights: HashMap<TaskId, u32>,
}

impl Measure {
  pub fn of<'a>(tasks: impl IntoIterator<Item = &'a Task>) -> Self {
    Self {
      unit: WorkUnit::Tasks,
      weights: tasks.into_iter().map(|task| (task.id(), 1)).collect(),
    }
  }

  /// The work `task` is; a task the measure was not taken of counts as one task.
  pub fn weight(&self, task: TaskId) -> u32 {
    self.weights.get(&task).copied().unwrap_or(1)
  }
}
