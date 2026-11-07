//! What the groups of the Review list are built from: the project's tasks by id, its database, and a row's fields that
//! come the same for every group.

use std::{collections::HashMap, path::Path};

use orchester_core::{
  id::{RunId, TaskId},
  review::ReviewThread,
  run::{Run, RunNumber},
  store::RunStore,
  task::Task,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::{ReviewRow, ReviewState};
use crate::tasks::{data::ProjectTasks, detail::run_store_error, merge_attempts::merges::run_size};

/// The reads every group makes.
pub(super) struct Reader<'a> {
  pub tasks: &'a ProjectTasks,
  pub db: ProjectDb,
  pub root: &'a Path,
  pub programs: &'a Programs,
  named: HashMap<TaskId, &'a Task>,
}

impl<'a> Reader<'a> {
  pub fn open(tasks: &'a ProjectTasks, (root, programs): (&'a Path, &'a Programs)) -> Result<Self, StoreError> {
    Ok(Self {
      tasks,
      db: ProjectDb::open(root)?,
      root,
      programs,
      named: tasks.tasks.iter().map(|task| (task.id(), task)).collect(),
    })
  }

  /// The listed task `id`, with the project's tasks (the ones kept *Not shared* are not listed).
  pub fn task(&self, id: TaskId) -> Option<&'a Task> {
    self.named.get(&id).copied()
  }

  /// `run`, which the project may no longer hold.
  pub fn run(&self, run: RunId) -> Result<Option<Run>, StoreError> {
    self.db.run(run).map_err(run_store_error)
  }

  /// The row of `task`'s `run` of that number, which stands at `state` since `since`; `stored` is the run as the
  /// project holds it.
  pub fn row(
    &self,
    (task, (run, number)): (&Task, (RunId, RunNumber)),
    stored: Option<&Run>,
    threads: &[ReviewThread],
    (state, since): (ReviewState, Timestamp),
  ) -> ReviewRow {
    ReviewRow {
      task: task.id(),
      key: task.key().clone(),
      title: task.details().title.as_str().into(),
      tag: task
        .organization()
        .tags()
        .first()
        .and_then(|first| self.tasks.tags.iter().find(|tag| tag.id() == *first))
        .map(|tag| tag.name().as_str().into()),
      run,
      number,
      state,
      open_threads: open(threads).count(),
      size: stored.and_then(|run| run_size((self.root, self.programs), run)),
      since,
    }
  }
}

/// The threads of `threads` that *Resolve* did not fold.
pub(super) fn open(threads: &[ReviewThread]) -> impl Iterator<Item = &ReviewThread> {
  threads.iter().filter(|thread| thread.resolved_at().is_none())
}
