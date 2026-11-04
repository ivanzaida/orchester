//! The runs' worktrees as the merge queue reaches them (ORC-684, `docs/architecture/remote-runners.md` rule 5): only
//! through the project's runner, which syncs the base branch into a run's worktree and runs Check again there
//! ([`MergeQueueCommand`]). The app asks the project's runner (`crate::runner::Runners::merge_queue_worktrees`);
//! tests hand in their own.

use orchester_ipc::merge_queue::{MergeQueueCommand, MergeQueueReply};

use crate::messages::Message;

/// Where the merge queue's work in a run's worktree is carried out.
pub trait RunWorktrees: Send + Sync {
  /// Carries `command` out in a run's worktree and answers how it went; why it could not be asked when it could not,
  /// which leaves the queue as it is to try again later. Blocks until it is done: called off the UI's thread, from
  /// the queue's blocking work.
  fn carry_out(&self, command: MergeQueueCommand) -> Result<MergeQueueReply, Message>;
}
