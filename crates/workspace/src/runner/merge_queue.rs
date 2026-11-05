//! The merge queue's work in a run's worktree, carried out by the project's runner ([`Command::MergeQueue`],
//! ORC-684): the desktop never runs git or the checks in a run's worktree itself.

use orchester_ipc::{
  Command, Reply,
  merge_queue::{MergeQueueCommand, MergeQueueReply},
};

use super::{RequestError, RunRequestError, Runners};
use crate::{merge_queue::RunWorktrees, messages::Message, tasks::run::control_message, workspace::OpenProject};

impl Runners {
  /// Asks `project`'s runner, starting it if needed, to carry the merge queue's `command` out in a run's worktree.
  pub async fn merge_queue_in(
    &self,
    project: &OpenProject,
    command: MergeQueueCommand,
  ) -> Result<MergeQueueReply, RunRequestError> {
    match self.ask_project(project, Command::MergeQueue(command)).await? {
      Reply::MergeQueue(reply) => Ok(reply),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to MergeQueue");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// The runs' worktrees of `project` as its merge queue reaches them: through its runner.
  pub fn merge_queue_worktrees(&self, project: &OpenProject) -> RunnerWorktrees {
    RunnerWorktrees {
      runners: self.clone(),
      project: project.clone(),
    }
  }
}

/// The runs' worktrees of one project, reached through its runner ([`Runners::merge_queue_worktrees`]).
#[derive(Clone)]
pub struct RunnerWorktrees {
  runners: Runners,
  project: OpenProject,
}

impl RunWorktrees for RunnerWorktrees {
  /// Waits on the runners' runtime for the runner's answer: called from the queue's blocking work, never from a task
  /// of that runtime.
  fn carry_out(&self, command: MergeQueueCommand) -> Result<MergeQueueReply, Message> {
    self
      .runners
      .inner
      .runtime
      .block_on(self.runners.merge_queue_in(&self.project, command))
      .map_err(|error| control_message(&error))
  }
}
