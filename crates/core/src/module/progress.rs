//! How far a module's work is now (ORC-48): Canceled tasks are out of its scope, and a task whose workflow status is
//! Done counts as done. The desktop's Modules list and a team server's module summaries count it by this rule.

use super::measure::{Measure, WorkUnit};
use crate::task::{Task, organization::WorkflowStatus};

/// How far a module's work is now: its scope (its tasks that are not Canceled) and what of it is done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
  pub unit: WorkUnit,
  pub scope: u32,
  pub done: u32,
}

impl Progress {
  /// The progress of the module's `tasks`, measured by `measure`.
  pub fn of<'a>(tasks: impl IntoIterator<Item = &'a Task>, measure: &Measure) -> Self {
    let (mut scope, mut done) = (0, 0);
    for task in tasks {
      let status = task.organization().status();
      if status == WorkflowStatus::Canceled {
        continue;
      }
      let weight = measure.weight(task.id());
      scope += weight;
      if status == WorkflowStatus::Done {
        done += weight;
      }
    }
    Self {
      unit: measure.unit,
      scope,
      done,
    }
  }

  pub const fn remaining(self) -> u32 {
    self.scope.saturating_sub(self.done)
  }

  /// What is done, in whole percent of the scope; `None` for a module without work in scope.
  pub fn percent(self) -> Option<u32> {
    (self.scope > 0).then(|| self.done * 100 / self.scope)
  }
}
