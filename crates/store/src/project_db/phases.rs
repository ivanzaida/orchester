//! Where each task of a project stands, read for All projects' run counts without the rest of
//! the task list.

use orchester_core::{
  run::TaskPhase,
  store::{MergeStore, RunStore, TaskStore},
};

use super::ProjectDb;
use crate::RunStoreError;

impl ProjectDb {
  /// The phase of each task from its latest run, merged or not. A task whose latest run cannot
  /// be read is left out: where it stands is unknown.
  pub fn task_phases(&self) -> Result<Vec<TaskPhase>, RunStoreError> {
    let mut phases = Vec::new();
    for task in self.tasks()? {
      let latest = match self.runs_of_task(task.id())?.pop() {
        Some(Ok(run)) => Some(run),
        Some(Err(_)) => continue,
        None => None,
      };
      let merge = match &latest {
        Some(run) => self.run_merge(run.id())?,
        None => None,
      };
      phases.push(TaskPhase::of(&task, latest.as_ref(), merge.as_ref()));
    }
    Ok(phases)
  }
}
