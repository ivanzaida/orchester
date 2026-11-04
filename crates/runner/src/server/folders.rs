//! The desktop's requests about the folders the project's runs leave with the executor
//! ([`Command::Folders`](orchester_ipc::Command::Folders)), answered once carried out.

use orchester_ipc::{Reply, folders::FoldersCommand};

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  pub(super) fn folders(&self, scheduler: Scheduler, command: FoldersCommand) -> Answer {
    self.reply(async move {
      match scheduler.folders(command).await {
        Ok(reply) => Reply::Folders(reply),
        Err(rejection) => Reply::Rejected(rejection),
      }
    })
  }
}
