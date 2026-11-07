//! What the lines about watching name beside what their entries keep, read from the project's tasks as the window
//! holds them: how many tasks a watched target holds now (*Webhooks v2 · 12 tasks*, `AoU7q` in `04` `OKtX2`), and the
//! module of a task another conversation took (*the rest of Billing stays here*, `Hd8Iq` in `avDZO` b). Both are
//! `orchester_workspace`'s [`WatchedTasks`]; they are read only for a conversation with such a line.

use orchester_core::{id::TaskId, orchestrator::WatchTarget};
use orchester_workspace::{orchestrator::watching::WatchedTasks, tasks::data::ProjectTasks};

/// The tasks of each target, and each task's module by name; `None` while the tasks are not read, or no line of the
/// conversation names them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::screens::orchestrator) struct WatchFacts(Option<WatchedTasks>);

impl WatchFacts {
  /// The facts of `tasks`, the project's tasks once they are read.
  pub(in crate::screens::orchestrator) fn of(tasks: Option<&ProjectTasks>) -> Self {
    Self(tasks.map(WatchedTasks::of))
  }

  /// How many tasks `target` holds now; `None` for a task, and while the tasks are not read.
  pub(in crate::screens::orchestrator) fn tasks_of(&self, target: WatchTarget) -> Option<usize> {
    self.0.as_ref()?.tasks_of(target)
  }

  /// The name of `task`'s module; `None` for a task without one or not read.
  pub(in crate::screens::orchestrator) fn module_of(&self, task: TaskId) -> Option<&str> {
    self.0.as_ref()?.module_of(task)
  }
}
