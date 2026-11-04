//! Story points estimated for the desktop ([`Command::EstimateTask`](orchester_ipc::Command::EstimateTask)),
//! answered once the estimate ended, without holding up the runner's other requests.

use orchester_core::id::TaskId;
use orchester_ipc::Reply;

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  /// Answers with the estimate of `task`'s story points once it ended.
  pub(super) fn estimate(&self, scheduler: Scheduler, task: TaskId) -> Answer {
    self.reply(async move {
      match scheduler.estimate_task(task).await {
        Ok(outcome) => Reply::TaskEstimated { task, outcome },
        Err(rejection) => Reply::Rejected(rejection),
      }
    })
  }
}
