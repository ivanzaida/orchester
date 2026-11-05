//! The merge queue's work in a run's worktree ([`Command::MergeQueue`](orchester_ipc::Command::MergeQueue)),
//! answered once it is done, without holding up the runner's other requests.

use orchester_ipc::{Reply, merge_queue::MergeQueueCommand};

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  /// Answers the merge queue's `command` once it is carried out in the run's worktree.
  pub(super) fn merge_queue(&self, scheduler: Scheduler, command: MergeQueueCommand) -> Answer {
    self.reply(async move {
      match scheduler.merge_queue(command).await {
        Ok(reply) => Reply::MergeQueue(reply),
        Err(rejection) => Reply::Rejected(rejection),
      }
    })
  }
}
