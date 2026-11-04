//! What a run's folder holds beyond its last commit, for the desktop's start-up recovery
//! ([`Command::RunFolderState`](orchester_ipc::Command::RunFolderState)), answered once git has read it, without
//! holding up the runner's other requests.

use orchester_core::id::RunId;

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  /// Answers with the state of `run`'s folder once it is read.
  pub(super) fn run_folder_state(&self, scheduler: Scheduler, run: RunId) -> Answer {
    self.reply(async move { scheduler.run_folder_state(run).await })
  }
}
