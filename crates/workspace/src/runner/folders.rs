//! The folders a project's runs leave with their executor — the warm workspaces and the worktrees runs made before
//! workspaces — as the desktop reaches them: through the project's runner ([`Command::Folders`],
//! `docs/architecture/remote-runners.md`, rule 5), never on disk itself.

use orchester_core::{run::BranchName, workspaces::WorkspaceNumber};
use orchester_ipc::{
  Command, Reply,
  folders::{ExecutorFolder, FolderList, FolderSizes, FoldersCommand, FoldersReply},
};

use super::{RequestError, RunRequestError, Runners};
use crate::workspace::OpenProject;

/// A project's executor folders, asked of its runner, which is started if needed. Each call blocks until the runner
/// answered: call it off the async runtime, as the blocking work around it already is.
#[derive(Clone)]
pub struct ExecutorFolders {
  runners: Runners,
  project: OpenProject,
}

impl ExecutorFolders {
  pub const fn new(runners: Runners, project: OpenProject) -> Self {
    Self { runners, project }
  }

  /// The sizes of warm workspaces `workspaces`, and every worktree a run made before workspaces with its size.
  pub fn sizes(&self, workspaces: Vec<WorkspaceNumber>) -> Result<FolderSizes, RunRequestError> {
    match self.ask(FoldersCommand::Sizes { workspaces })? {
      FoldersReply::Sizes(sizes) => Ok(sizes),
      other => unexpected(&other),
    }
  }

  /// Which of warm workspaces `workspaces` have a folder, and the names of the worktrees folder's folders, unsized.
  pub fn list(&self, workspaces: Vec<WorkspaceNumber>) -> Result<FolderList, RunRequestError> {
    match self.ask(FoldersCommand::List { workspaces })? {
      FoldersReply::Listed(list) => Ok(list),
      other => unexpected(&other),
    }
  }

  /// The size of `folder`; `None` when the runner could not read it.
  pub fn measure(&self, folder: ExecutorFolder) -> Result<Option<u64>, RunRequestError> {
    match self.ask(FoldersCommand::Measure { folder })? {
      FoldersReply::Measured { bytes } => Ok(bytes),
      other => unexpected(&other),
    }
  }

  /// Deletes every ignored file of idle workspace `number`, reserved by the caller; the bytes that freed.
  pub fn clean(&self, number: WorkspaceNumber) -> Result<u64, RunRequestError> {
    match self.ask(FoldersCommand::Clean { number })? {
      FoldersReply::Cleaned { freed } => Ok(freed),
      other => unexpected(&other),
    }
  }

  /// Throws away the uncommitted changes of stranded workspace `number`, reserved by the caller.
  pub fn discard(&self, number: WorkspaceNumber) -> Result<(), RunRequestError> {
    match self.ask(FoldersCommand::Discard { number })? {
      FoldersReply::Discarded => Ok(()),
      other => unexpected(&other),
    }
  }

  /// Commits the uncommitted changes of stranded workspace `number`, reserved by the caller, on the new `branch`.
  pub fn keep(&self, number: WorkspaceNumber, branch: BranchName) -> Result<(), RunRequestError> {
    match self.ask(FoldersCommand::Keep { number, branch })? {
      FoldersReply::Kept => Ok(()),
      other => unexpected(&other),
    }
  }

  fn ask(&self, command: FoldersCommand) -> Result<FoldersReply, RunRequestError> {
    let asked = self.runners.ask_project(&self.project, Command::Folders(command));
    match self.runners.inner.runtime.block_on(asked)? {
      Reply::Folders(reply) => Ok(reply),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to a Folders request");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }
}

/// A reply of another request's kind: the runner and the desktop disagree, so the outcome is unknown, as for a lost
/// reply.
fn unexpected<T>(reply: &FoldersReply) -> Result<T, RunRequestError> {
  tracing::error!(?reply, "unexpected reply to a Folders request");
  Err(RunRequestError::Request(RequestError::Lost))
}
