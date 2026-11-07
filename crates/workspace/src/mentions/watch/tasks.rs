//! The /watch-task picker (frame 03 `mQdxa`): the tasks whose key or title holds what was typed, ignoring case, as
//! the @ menu finds them ([`crate::mentions::catalog`]), those left to watch first, each most recently changed first.
//! Each says its module, its workflow status and who leads it, itself or through a less specific subscription
//! that covers it: its module's (or that of the tasks without a module), else the project's.

use std::{cmp::Reverse, sync::Arc};

use orchester_core::{
  id::{ModuleId, TaskId},
  orchestrator::{
    WatchTarget,
    watch::{EndReason, TargetTasks},
  },
  project::TaskKey,
  task::organization::WorkflowStatus,
};

use super::{Choosing, Leader, WatchCatalog, faded};
use crate::mentions::catalog::task_matches;

/// The module a task is in, as its row names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InModule {
  pub id: ModuleId,
  pub name: Arc<str>,
}

/// A row of the task picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskEntry {
  pub id: TaskId,
  pub key: TaskKey,
  pub title: Arc<str>,
  /// `None`: it has none, or the project no longer keeps the one it names.
  pub module: Option<InModule>,
  pub status: WorkflowStatus,
  pub leader: Leader,
  /// Why it is offered faded: it is Done or Canceled, so there is nothing to watch.
  pub faded: Option<EndReason>,
}

impl WatchCatalog {
  /// The task picker's rows for `query`, typed after the command, in `choosing`.
  pub fn tasks(&self, query: &str, choosing: Choosing<'_>) -> Vec<TaskEntry> {
    let query = query.to_lowercase();
    let mut matching: Vec<_> = self
      .tasks
      .iter()
      .filter(|task| task_matches(&query, &task.key, &task.title))
      .collect();
    matching.sort_by_key(|task| Reverse(task.at));
    let (mut rows, faded_rows): (Vec<TaskEntry>, Vec<TaskEntry>) = matching
      .into_iter()
      .map(|task| {
        let [own, outer @ ..] = WatchTarget::of_task(task.id, task.module);
        TaskEntry {
          id: task.id,
          key: task.key.clone(),
          title: task.title.clone(),
          module: task.module.and_then(|id| self.module(id)).map(|module| InModule {
            id: module.id(),
            name: module.name().as_str().into(),
          }),
          status: task.status,
          leader: self.leaders.of(own, &outer, choosing),
          faded: faded(TargetTasks::Task(task.status)),
        }
      })
      .partition(|row| row.faded.is_none());
    rows.extend(faded_rows);
    rows
  }
}
