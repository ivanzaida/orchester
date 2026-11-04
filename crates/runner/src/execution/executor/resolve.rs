//! A person's resolving by hand in a run's worktree, served where the worktree is
//! (`docs/architecture/remote-runners.md`, rule 5): the desktop keeps the project's record of the resolution and asks
//! for each git command in the worktree here ([`orchester_core::integration::resolve::request`]).

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::resolve::request::{self, FailureReason, ResolveAnswer, ResolveFailure, ResolveRequest},
};
use orchester_exec::programs::Programs;

use super::super::Scheduler;
use crate::execution_root::ExecutionRoot;

/// The git program, found on the search path.
const GIT: &str = "git";

impl Scheduler {
  /// Carries out `request` in `run`'s worktree, with git run as the desktop ran it there before, off the runtime's
  /// async workers: git may take long (a merge runs the repository's hooks).
  pub async fn resolve(&self, run: RunId, request: ResolveRequest) -> Result<ResolveAnswer, ResolveFailure> {
    let worktree = ExecutionRoot::local(&self.inner.project_root).run_worktree(run);
    let served = tokio::task::spawn_blocking(move || {
      let programs = Programs::processes();
      request::serve(
        (&programs.runner(), &programs.git_work()),
        Path::new(GIT),
        &worktree,
        &request,
      )
    })
    .await;
    served.unwrap_or_else(|error| {
      tracing::error!(%run, %error, "resolving in the run's worktree ended abnormally");
      Err(ResolveFailure {
        reason: FailureReason::GitStopped,
        detail: error.to_string(),
      })
    })
  }
}
