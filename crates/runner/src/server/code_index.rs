//! [`Command::RebuildCodeIndex`](orchester_ipc::Command::RebuildCodeIndex), answered once the index is built: the
//! request counts as the runner's work until then, so a runner no tab shows does not stop in the middle of it.

use orchester_core::revision::Revision;
use orchester_ipc::Reply;

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  /// Answers once the index of `revision` is built again, or with why it was not.
  pub(super) fn rebuild_code_index(&self, scheduler: Scheduler, revision: Revision) -> Answer {
    self.reply(async move {
      match scheduler.rebuild_code_index(revision.clone()).await {
        Ok(()) => Reply::CodeIndexRebuilt { revision },
        Err(rejection) => Reply::Rejected(rejection),
      }
    })
  }
}
