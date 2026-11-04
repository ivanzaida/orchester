//! Story points estimated for the desktop ([`orchester_ipc::Command::EstimateTask`]): on auto mode's reviewer, beside
//! the runs, taking no agent slot.

use orchester_core::{id::TaskId, task::estimate::EstimateOutcome};
use orchester_ipc::Rejection;

use super::Scheduler;
use crate::execution::executor::step::worker::estimate_task;

impl Scheduler {
  /// Estimates the story points of `task`, unless a person set them or nothing they are made from changed. The
  /// reviewer's settings and variables are asked through a link of the estimate's own.
  pub async fn estimate_task(&self, task: TaskId) -> Result<EstimateOutcome, Rejection> {
    let link = self.open_link().map_err(|error| Rejection::Failed {
      detail: error.to_string(),
    })?;
    estimate_task(
      (self.inner.project_root.clone(), link),
      self.inner.computer.clone(),
      task,
    )
    .await
  }
}
