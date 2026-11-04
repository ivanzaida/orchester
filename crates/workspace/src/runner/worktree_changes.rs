//! What a run's worktree holds beyond its checked-out commit, as the project's runner reads it
//! ([`Command::ReadWorktreeChanges`]): the desktop reaches a run's worktree only through the runner
//! (`docs/architecture/remote-runners.md`, rule 5).

use orchester_core::id::RunId;
use orchester_ipc::{Command, Reply, WorktreeChangesRead};

use super::{RequestError, RunRequestError, Runners};
use crate::workspace::OpenProject;

impl Runners {
  /// Asks `project`'s runner, starting it and waiting for it to attach as opening the project does, what the
  /// worktree of `run` holds beyond its commit. A project no tab shows lets its runner stop again once it has no work.
  pub async fn worktree_changes(
    &self,
    project: &OpenProject,
    run: RunId,
  ) -> Result<WorktreeChangesRead, RunRequestError> {
    match self.ask_project(project, Command::ReadWorktreeChanges { run }).await? {
      Reply::WorktreeChanges {
        run: read,
        read: changes,
      } if read == run => Ok(changes),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to ReadWorktreeChanges");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }
}
