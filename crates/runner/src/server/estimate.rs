//! The desktop's request for a task's story points ([`Command::EstimateTask`](orchester_ipc::Command::EstimateTask)),
//! answered once estimated.

use orchester_core::id::TaskId;
use orchester_ipc::Reply;

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  pub(super) fn estimate(&self, scheduler: Scheduler, task: TaskId) -> Answer {
    self.reply(async move {
      match scheduler.estimate_task(task).await {
        Ok(outcome) => Reply::TaskEstimated { task, outcome },
        Err(rejection) => Reply::Rejected(rejection),
      }
    })
  }
}
