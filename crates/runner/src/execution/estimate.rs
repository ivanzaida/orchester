//! Story points estimated for the desktop ([`orchester_ipc::Command::EstimateTask`]): on auto mode's reviewer, beside
//! the runs, taking no agent slot.

use orchester_core::{id::TaskId, task::estimate::EstimateOutcome};
use orchester_ipc::Rejection;

use super::{Scheduler, step::worker::estimate_task};

impl Scheduler {
  /// Estimates the story points of `task`, unless a person set them or nothing they are made from changed.
  pub async fn estimate_task(&self, task: TaskId) -> Result<EstimateOutcome, Rejection> {
    estimate_task(self.inner.project_root.clone(), self.inner.vault.clone(), task).await
  }
}
