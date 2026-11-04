//! [`Command::EstimateTask`](orchester_ipc::Command::EstimateTask), answered once the estimate ended.

use orchester_core::id::TaskId;
use orchester_ipc::Reply;

use super::{Answer, Later};
use crate::execution::Scheduler;

pub(super) fn estimate(later: &Later<'_>, scheduler: Scheduler, task: TaskId) -> Answer {
  later.reply(async move {
    match scheduler.estimate_task(task).await {
      Ok(outcome) => Reply::TaskEstimated { task, outcome },
      Err(rejection) => Reply::Rejected(rejection),
    }
  })
}
