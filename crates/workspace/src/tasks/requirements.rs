//! What the task page's Details reads of a task's requirements beside the task: the version kept when it
//! was created and the one its latest run started with (`orchester_core::task::requirements`).

use orchester_core::{
  run::Run,
  task::{
    Task,
    requirements::{LatestRun, RunStart, TaskRequirements},
  },
};
use orchester_store::{ProjectDb, StoreError};

use super::detail::TaskDetail;

impl TaskDetail {
  /// The task never had a run, readable or not: its Summary has nothing to show yet.
  pub fn before_first_run(&self) -> bool {
    self.run.is_none()
      && self.latest_unreadable.is_none()
      && self.history.is_empty()
      && self.earlier_unreadable.is_empty()
  }
}

/// `task`'s kept versions and its current one; `latest` is its latest run when it can be read, and
/// `unreadable` whether the latest one cannot.
pub(super) fn read(
  db: &ProjectDb,
  task: &Task,
  (latest, unreadable): (Option<&Run>, bool),
) -> Result<TaskRequirements, StoreError> {
  let creation = db.task_creation_details(task.id())?;
  let latest = match latest {
    Some(run) => LatestRun::Run(RunStart {
      run: run.id(),
      number: run.number(),
      details: db.run_start_details(run.id())?,
    }),
    None if unreadable => LatestRun::Unknown,
    None => LatestRun::None,
  };
  Ok(TaskRequirements::new(task.details().clone(), creation, latest))
}
