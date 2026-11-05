use orchester_core::id::TaskId;
use orchester_ipc::Reply;

use super::{Answer, Later, Scheduler};

/// Estimates the story points of `task` without holding up the runner's other requests.
pub(super) fn answer(later: &Later<'_>, handle: Scheduler, task: TaskId) -> Answer {
  later.reply(async move {
    match handle.estimate_task(task).await {
      Ok(outcome) => Reply::TaskEstimated { task, outcome },
      Err(rejection) => Reply::Rejected(rejection),
    }
  })
}
