//! The /watch-module picker (frame 02 `c8pALl`): the open modules whose name holds what was typed, ignoring case,
//! those with a task left to watch first, each in the project's order; then *Tasks without a module* and
//! *Everything in the project*, which the picker names and offers whatever was typed.

use std::sync::Arc;

use orchester_core::{
  module::ModuleState,
  orchestrator::{
    WatchTarget,
    watch::{EndReason, TargetTasks},
  },
  task::organization::WorkflowStatus,
};

use super::{Choosing, Leader, WatchCatalog, WatchTask, faded};
use crate::mentions::catalog::matches;

/// How many tasks a target takes in, how many of them run, and of how many something waits for the person.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
  /// Done and Canceled ones too.
  pub tasks: usize,
  pub running: usize,
  pub waiting: usize,
}

impl Counts {
  fn of<'a>(tasks: impl Iterator<Item = &'a WatchTask>) -> Self {
    tasks.fold(Self::default(), |counts, task| Self {
      tasks: counts.tasks + 1,
      running: counts.running + usize::from(task.running),
      waiting: counts.waiting + usize::from(task.waits),
    })
  }
}

/// A row of the module picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleEntry {
  /// A module, the tasks without a module, or the whole project.
  pub target: WatchTarget,
  /// The module's name; `None` for the two others, which the picker names.
  pub name: Option<Arc<str>>,
  pub counts: Counts,
  pub leader: Leader,
  /// Why it is offered faded: every task of the module is Done or Canceled (*Nothing left to watch*).
  pub faded: Option<EndReason>,
}

impl WatchCatalog {
  /// The module picker's rows for `query`, typed after the command, in `choosing`.
  pub fn modules(&self, query: &str, choosing: Choosing<'_>) -> Vec<ModuleEntry> {
    let query = query.to_lowercase();
    let (mut rows, faded_rows): (Vec<ModuleEntry>, Vec<ModuleEntry>) = self
      .modules
      .iter()
      .filter(|module| module.details().state() == ModuleState::Open)
      .filter(|module| matches(&query, &[module.name().as_str()]))
      .map(|module| {
        let members: Vec<&WatchTask> = self.tasks.iter().filter(|task| task.module == Some(module.id())).collect();
        let statuses: Vec<WorkflowStatus> = members.iter().map(|task| task.status).collect();
        let target = WatchTarget::Module(module.id());
        ModuleEntry {
          target,
          name: Some(module.name().as_str().into()),
          counts: Counts::of(members.into_iter()),
          leader: self.leaders.of(target, &[], choosing),
          faded: faded(TargetTasks::Module(&statuses)),
        }
      })
      .partition(|row| row.faded.is_none());
    rows.extend(faded_rows);
    rows.push(self.whole(
      WatchTarget::WithoutModule,
      self.tasks.iter().filter(|task| task.module.is_none()),
      choosing,
    ));
    rows.push(self.whole(WatchTarget::Project, self.tasks.iter(), choosing));
    rows
  }

  /// The row of `target`, the tasks without a module or the whole project, which takes in `tasks`.
  fn whole<'a>(
    &self,
    target: WatchTarget,
    tasks: impl Iterator<Item = &'a WatchTask>,
    choosing: Choosing<'_>,
  ) -> ModuleEntry {
    ModuleEntry {
      target,
      name: None,
      counts: Counts::of(tasks),
      leader: self.leaders.of(target, &[], choosing),
      faded: None,
    }
  }
}
