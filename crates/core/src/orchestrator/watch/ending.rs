//! When a subscription ends by itself. A module's ends once every task of it is Done or Canceled, and it has at least
//! one; a task's ends once its task is Done or Canceled. Tasks without a module and the whole project never end by
//! themselves: only the person stops them. The reason goes with the end, so the one turn that writes its summary can
//! say why.

use crate::task::organization::WorkflowStatus;

/// A subscription's target with the workflow statuses of the tasks it takes in now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetTasks<'a> {
  Task(WorkflowStatus),
  /// The statuses of every task of the module.
  Module(&'a [WorkflowStatus]),
  WithoutModule,
  Project,
}

impl TargetTasks<'_> {
  /// Whether the subscription ends now, and why.
  pub fn ending(self) -> Ending {
    match self {
      Self::Task(WorkflowStatus::Done) => Ending::Ends(EndReason::TaskDone),
      Self::Task(WorkflowStatus::Canceled) => Ending::Ends(EndReason::TaskCanceled),
      Self::Task(_) => Ending::Open,
      Self::Module(statuses) => module_ending(statuses),
      Self::WithoutModule => Ending::Never(NeverEnds::WithoutModule),
      Self::Project => Ending::Never(NeverEnds::Project),
    }
  }
}

fn module_ending(statuses: &[WorkflowStatus]) -> Ending {
  if statuses.is_empty() || !statuses.iter().all(|status| status.is_closed()) {
    return Ending::Open;
  }
  let done = statuses
    .iter()
    .filter(|status| **status == WorkflowStatus::Done)
    .count();
  Ending::Ends(EndReason::ModuleClosed {
    done,
    canceled: statuses.len() - done,
  })
}

/// Whether a subscription ends by itself now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
  /// It goes on: a task it takes in is still open, or its module has no task yet.
  Open,
  /// It ends now, with one summary turn for this reason.
  Ends(EndReason),
  /// Its target never ends by itself; only the person stops it.
  Never(NeverEnds),
}

/// Why a subscription ended by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
  TaskDone,
  TaskCanceled,
  /// Every task of the module is Done or Canceled: this many of each.
  ModuleClosed {
    done: usize,
    canceled: usize,
  },
}

/// Why a subscription never ends by itself: its target takes in tasks yet to come.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeverEnds {
  WithoutModule,
  Project,
}
