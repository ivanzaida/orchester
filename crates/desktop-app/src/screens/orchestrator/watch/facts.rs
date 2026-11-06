//! What the lines about watching name beside what their entries keep, read from the project's tasks as the window
//! holds them: how many tasks a watched target holds now (*Webhooks v2 · 12 tasks*, `AoU7q` in `04` `OKtX2`), and the
//! module of a task another conversation took (*the rest of Billing stays here*, `Hd8Iq` in `avDZO` b).

use std::{collections::BTreeMap, sync::Arc};

use orchester_core::{
  id::{ModuleId, TaskId},
  orchestrator::WatchTarget,
};
use orchester_workspace::tasks::data::ProjectTasks;

/// The tasks of each target, and each task's module by name; empty while the tasks are not read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::screens::orchestrator) struct WatchFacts {
  /// The tasks are read: the counts below say how many there are.
  read: bool,
  modules: BTreeMap<ModuleId, usize>,
  without_module: usize,
  all: usize,
  task_modules: BTreeMap<TaskId, Arc<str>>,
}

impl WatchFacts {
  /// The facts of `tasks`, the project's tasks once they are read.
  pub(in crate::screens::orchestrator) fn of(tasks: Option<&ProjectTasks>) -> Self {
    let Some(tasks) = tasks else {
      return Self::default();
    };
    let names: BTreeMap<ModuleId, Arc<str>> = tasks
      .modules
      .iter()
      .map(|module| (module.id(), Arc::from(module.name().as_str())))
      .collect();
    let mut facts = Self {
      read: true,
      all: tasks.tasks.len(),
      ..Self::default()
    };
    for task in tasks.tasks.iter() {
      match task.module() {
        Some(module) => {
          *facts.modules.entry(module).or_default() += 1;
          if let Some(name) = names.get(&module) {
            facts.task_modules.insert(task.id(), name.clone());
          }
        }
        None => facts.without_module += 1,
      }
    }
    facts
  }

  /// How many tasks `target` holds now; `None` for a task, and while the tasks are not read.
  pub(in crate::screens::orchestrator) fn tasks_of(&self, target: WatchTarget) -> Option<usize> {
    if !self.read {
      return None;
    }
    match target {
      WatchTarget::Task(_) => None,
      WatchTarget::Module(module) => Some(self.modules.get(&module).copied().unwrap_or_default()),
      WatchTarget::WithoutModule => Some(self.without_module),
      WatchTarget::Project => Some(self.all),
    }
  }

  /// The name of `task`'s module; `None` for a task without one or not read.
  pub(in crate::screens::orchestrator) fn module_of(&self, task: TaskId) -> Option<&str> {
    self.task_modules.get(&task).map(|name| &**name)
  }
}
