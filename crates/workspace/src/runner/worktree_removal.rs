//! Removing what a run left through the project's runner ([`Command::RemoveWorktrees`]), which removes its worktrees,
//! stops its processes in a held folder, prunes the codebase indexes and sweeps the worktree folders where they are
//! (`docs/architecture/remote-runners.md`, rule 5). A test whose session has no runner has the requests carried out in
//! its own process instead ([`Runners::serve_worktrees_here_for_test`]).

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::{
    Cleanup,
    removal::{RemovalFailure, RemovalRequest},
  },
};
#[cfg(feature = "test-support")]
use orchester_exec::programs::Programs;
use orchester_ipc::{Command, Reply};

use super::{RequestError, RunRequestError, Runners};

impl Runners {
  /// Removes what run `run` left, as `request` allows, through the runner of the project at `root`, named `name`,
  /// which starts and attaches as opening the project does when it is not running. Blocks until the runner answered:
  /// call it off the runtime's async workers.
  pub(crate) fn remove_worktrees_blocking(
    &self,
    (root, name): (&Path, &str),
    run: RunId,
    request: RemovalRequest,
  ) -> Result<Result<Cleanup, RemovalFailure>, RunRequestError> {
    let command = Command::RemoveWorktrees { run, request };
    match self.inner.runtime.block_on(self.ask_named((root, name), command))? {
      Reply::WorktreesRemoved(cleanup) => Ok(Ok(cleanup)),
      Reply::RemovalFailed(failure) => Ok(Err(failure)),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to RemoveWorktrees");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// The session of a test has no runner: what the desktop asks of a run's worktrees is carried out in this process
  /// with `programs`, through the same serialized messages (`crate::tasks::worktree_removal::WorktreeRemover`).
  #[cfg(feature = "test-support")]
  pub fn serve_worktrees_here_for_test(&self, programs: Programs) {
    *self.inner.worktrees_here.lock() = Some(programs);
  }

  /// The programs [`Runners::serve_worktrees_here_for_test`] was given.
  #[cfg(feature = "test-support")]
  pub(crate) fn worktrees_here(&self) -> Option<Programs> {
    self.inner.worktrees_here.lock().clone()
  }
}
