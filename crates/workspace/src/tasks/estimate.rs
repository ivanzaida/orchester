//! Story points (`orchester_core::task::estimate`) as the screens and automation set and ask for them: a person's
//! points from the task form, automation's through `create_task` / `update_task` (never over a person's), and
//! Orchester's own estimate, which a project's runner makes once a task is saved, while *Let Orchester estimate story
//! points* is on (Settings › What automation may do; off until a person turns it on).

use std::path::Path;

use orchester_core::{
  id::TaskId,
  store::TaskStore,
  task::{
    Task,
    estimate::{
      EstimateBasis, EstimateOutcome, EstimateSource, PointsRefused, StoryEstimate, StoryPoints, check_automation,
    },
  },
  time::Timestamp,
};
use orchester_store::{AutomationPoints, ProjectDb, StoreError};

use super::outside::{Outside, OutsideChanges};
use crate::{
  runner::Runners,
  workspace::{OpenProject, Workspace},
};

/// Why points were not set.
#[derive(Debug, thiserror::Error)]
pub enum PointsError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Refused(#[from] PointsRefused),
}

/// Sets automation's `points` for `task`, as it is stored now, unless a person set its points. They count as made
/// from its brief and criteria as they are, so Orchester does not estimate it again until they change.
pub fn set_automation_points(db: &mut ProjectDb, task: &Task, points: StoryPoints) -> Result<(), PointsError> {
  check_automation(task.estimate())?;
  let estimate = StoryEstimate::automation(points, None, EstimateBasis::of(task.details()), Timestamp::now());
  match db.set_automation_points(task.id(), &estimate)? {
    AutomationPoints::Stored => Ok(()),
    AutomationPoints::PersonsValue => Err(PointsRefused::PersonsValue.into()),
    // `task` was read before its brief or criteria changed again: the points are the caller's all the same.
    AutomationPoints::Outdated => {
      let current = db.task(task.id())?.ok_or(StoreError::TaskNotFound(task.id()))?;
      set_automation_points(db, &current, points)
    }
  }
}

/// Stores a person's `points` for the saved `task` of the project at `repo_root` (`None`: Automatic) when they
/// differ from the person's points it has; the task comes back with its points as stored now.
pub fn apply_person_points(repo_root: &Path, task: Task, points: Option<StoryPoints>) -> Result<Task, StoreError> {
  let person = task
    .estimate()
    .filter(|estimate| estimate.source() == EstimateSource::Person)
    .map(StoryEstimate::points);
  if person == points {
    return Ok(task);
  }
  let mut db = ProjectDb::open(repo_root)?;
  db.set_person_points(task.id(), points, Timestamp::now())?;
  let estimate = db.story_estimate(task.id())?;
  Ok(task.with_estimate(estimate))
}

/// Asks a project's runner for Orchester's estimate of a saved task while the person lets Orchester estimate.
#[derive(Clone)]
pub struct Estimates {
  workspace: Workspace,
  runners: Runners,
  changes: OutsideChanges,
}

impl Estimates {
  pub const fn new(workspace: Workspace, runners: Runners, changes: OutsideChanges) -> Self {
    Self {
      workspace,
      runners,
      changes,
    }
  }

  /// Asks for the estimate of `task` of `project`, without waiting for it: nothing while the setting is off; the
  /// runner asks no model for a task a person sized or whose brief and criteria did not change. Points it gives
  /// are shown as a change of the project's tasks.
  pub fn request(&self, project: OpenProject, task: TaskId) {
    let this = self.clone();
    self.runners.spawn(async move {
      let workspace = this.workspace.clone();
      match crate::background::run(move || workspace.estimates_story_points()).await {
        Ok(Ok(true)) => {}
        Ok(Ok(false)) => return,
        Ok(Err(error)) => {
          tracing::warn!(%error, "cannot read whether Orchester estimates story points");
          return;
        }
        // Logged by `background::run`.
        Err(_) => return,
      }
      match this.runners.estimate_task_in(&project, task).await {
        Ok(EstimateOutcome::Estimated { points }) => {
          tracing::info!(%task, points = points.get(), "story points estimated");
          this.changes.record(project.project.id(), Outside::Tasks);
        }
        Ok(outcome) => tracing::debug!(%task, ?outcome, "no story points estimated"),
        Err(error) => tracing::warn!(%task, %error, "the story-point estimate was not made"),
      }
    });
  }
}
