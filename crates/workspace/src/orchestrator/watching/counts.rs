//! How many tasks each target a conversation can watch takes in now, as the lines about watching name them
//! (*Leading Webhooks v2 · 12 tasks*, `04` `OKtX2`), and the module of each task (*the rest of Billing stays here*,
//! `avDZO` b). A module counts as the Modules list counts it ([`module_tasks`]); the tasks without a module and the
//! whole project as an event's routing covers them ([`WatchTarget::covers`]).

use std::{collections::BTreeMap, sync::Arc};

use orchester_core::{
  id::{ModuleId, TaskId},
  orchestrator::WatchTarget,
};

use crate::{modules::data::module_tasks, tasks::data::ProjectTasks};

/// The tasks of each target a project's conversations can watch, and each task's module by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WatchedTasks {
  modules: BTreeMap<ModuleId, usize>,
  without_module: usize,
  all: usize,
  task_modules: BTreeMap<TaskId, Arc<str>>,
}

impl WatchedTasks {
  /// What `tasks`, a project's tasks with its modules, give each target now.
  pub fn of(tasks: &ProjectTasks) -> Self {
    let covered = |target: WatchTarget| {
      tasks
        .tasks
        .iter()
        .filter(|task| target.covers(task.id(), task.module()))
        .count()
    };
    let names: BTreeMap<ModuleId, Arc<str>> = tasks
      .modules
      .iter()
      .map(|module| (module.id(), Arc::from(module.name().as_str())))
      .collect();
    Self {
      modules: names
        .keys()
        .map(|&module| (module, module_tasks(module, &tasks.tasks).count()))
        .collect(),
      without_module: covered(WatchTarget::WithoutModule),
      all: covered(WatchTarget::Project),
      task_modules: tasks
        .tasks
        .iter()
        .filter_map(|task| Some((task.id(), names.get(&task.module()?)?.clone())))
        .collect(),
    }
  }

  /// How many tasks `target` takes in now; `None` for a task, which is one. A module gone from the project has none.
  pub fn tasks_of(&self, target: WatchTarget) -> Option<usize> {
    match target {
      WatchTarget::Task(_) => None,
      WatchTarget::Module(module) => Some(self.modules.get(&module).copied().unwrap_or_default()),
      WatchTarget::WithoutModule => Some(self.without_module),
      WatchTarget::Project => Some(self.all),
    }
  }

  /// The name of `task`'s module; `None` for a task without one, or not among the tasks.
  pub fn module_of(&self, task: TaskId) -> Option<&str> {
    self.task_modules.get(&task).map(|name| &**name)
  }
}
