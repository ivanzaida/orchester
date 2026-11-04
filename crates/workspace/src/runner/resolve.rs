//! Resolving by hand in a run's worktree through the project's runner ([`Command::Resolve`]), which carries each
//! request out where the worktree is (`docs/architecture/remote-runners.md`, rule 5). A test whose session has no
//! runner has them carried out in its own process instead ([`Runners::serve_worktrees_here_for_test`]).

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::resolve::request::{ResolveAnswer, ResolveFailure, ResolveRequest},
};
#[cfg(feature = "test-support")]
use orchester_exec::programs::Programs;
use orchester_ipc::{Command, Reply};

use super::{RequestError, RunRequestError, Runners};

impl Runners {
  /// Carries out `request` in run `run`'s worktree through the runner of the project at `root`, named `name`, which
  /// starts if it is not running. Blocks until the runner answered: call it off the runtime's async workers.
  pub(crate) fn resolve_blocking(
    &self,
    (root, name): (&Path, &str),
    run: RunId,
    request: ResolveRequest,
  ) -> Result<Result<ResolveAnswer, ResolveFailure>, RunRequestError> {
    let command = Command::Resolve { run, request };
    match self.inner.runtime.block_on(self.ask_named((root, name), command))? {
      Reply::Resolved(answer) => Ok(Ok(answer)),
      Reply::ResolveFailed(failure) => Ok(Err(failure)),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to Resolve");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// The session of a test has no runner: what the desktop asks of a run's worktrees (resolving by hand, removing them)
  /// is carried out in this process with `programs`, through the same serialized messages
  /// (`crate::tasks::resolve::RunWorktrees`, `crate::tasks::worktree_removal::WorktreeRemover`).
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
