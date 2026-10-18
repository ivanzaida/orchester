//! What the task page's Details reads of a task's requirements beside the task: the version kept when it
//! was created and the one its latest run started with (`orchester_core::task::requirements`).

use orchester_core::{
  run::{Run, RunNumber},
  task::{
    Task,
    requirements::{LatestRun, RunStart, TaskRequirements},
  },
};
use orchester_store::{ProjectDb, SharedTaskInfo, StoreError};

use super::detail::TaskDetail;

impl TaskDetail {
  /// The task never had a run, readable or not, here or on a teammate's computer: its Summary has nothing
  /// to show yet.
  pub fn before_first_run(&self) -> bool {
    self.run.is_none()
      && self.latest_unreadable.is_none()
      && self.history.is_empty()
      && self.earlier_unreadable.is_empty()
      && !self.requirements.latest_run().ran()
  }
}

/// The task's runs on this computer: its latest one when it can be read, and whether the latest one
/// cannot.
#[derive(Clone, Copy)]
pub(super) struct LocalRuns<'a> {
  pub latest: Option<&'a Run>,
  pub unreadable: bool,
}

/// `task`'s kept versions and its current one. Without a run on this computer, the run its holder reported
/// through `shared`'s server is its latest.
pub(super) fn read(
  db: &ProjectDb,
  task: &Task,
  local: LocalRuns<'_>,
  shared: Option<&SharedTaskInfo>,
) -> Result<TaskRequirements, StoreError> {
  let creation = db.task_creation_details(task.id())?;
  let reported = shared.and_then(|info| info.run.as_ref());
  let latest = match (local.latest, reported) {
    (Some(run), _) => LatestRun::Run(RunStart {
      run: run.id(),
      number: run.number(),
      details: db.run_start_details(run.id())?,
    }),
    (None, _) if local.unreadable => LatestRun::Unknown,
    (None, Some(reported)) => LatestRun::Elsewhere(RunNumber::new(reported.number)),
    (None, None) => LatestRun::None,
  };
  Ok(TaskRequirements::new(task.details().clone(), creation, latest))
}
