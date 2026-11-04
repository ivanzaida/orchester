//! The Orchestrator chat's git in a run's worktree, run here for the desktop ([`orchester_ipc::Command::RunGit`],
//! `docs/architecture/remote-runners.md` rule 5): the desktop never runs git there itself. The run's worktree belongs
//! to its agents until the run ended, so until then only reads run ([`RunGitRequest::allowed_before_end`]); the
//! worktree is the run's own, found here, never a folder the desktop names.

use std::{future::Future, path::Path};

use orchester_core::{
  integration::WORKTREES_DIR,
  orchestrator::run_git::{RunGitAnswer, RunGitRequest},
  store::{RunStore, WorktreeStore},
};
use orchester_exec::{programs::Programs, run_git::serve};
use orchester_ipc::{Rejection, Reply};
use orchester_store::ProjectDb;

use super::super::{Scheduler, failed};

impl Scheduler {
  /// Carries out `request` in its run's worktree, off the async workers; answered with [`Reply::RunGit`] or why not.
  pub fn run_git(&self, request: RunGitRequest) -> impl Future<Output = Reply> + Send + 'static {
    let root = self.inner.project_root.clone();
    async move {
      let answered = tokio::task::spawn_blocking(move || answer(&root, &request)).await;
      match answered {
        Ok(Ok(answer)) => Reply::RunGit(answer),
        Ok(Err(rejection)) => Reply::Rejected(rejection),
        Err(error) => Reply::Rejected(failed("the chat's git did not finish", &error)),
      }
    }
  }
}

/// `request` carried out in the project at `root`, unless its run is unknown, has not ended and the request does not
/// only read, or has no worktree any more. Blocks on SQLite and git.
fn answer(root: &Path, request: &RunGitRequest) -> Result<RunGitAnswer, Rejection> {
  let db = ProjectDb::open(root).map_err(|error| failed("cannot open the project database", &error))?;
  let run = db
    .run(request.run)
    .map_err(|error| failed("cannot read the run", &error))?
    .ok_or(Rejection::RunNotFound)?;
  if !run.status().is_terminal() && !request.allowed_before_end() {
    return Err(Rejection::RunNotEnded);
  }
  let dir = root.join(WORKTREES_DIR).join(run.id().to_string());
  let removed = db
    .worktree_removal(run.id())
    .map_err(|error| failed("cannot read the worktree's removal", &error))?;
  if removed.is_some() || !dir.is_dir() {
    return Err(Rejection::WorktreeRemoved);
  }
  Ok(serve(&Programs::processes(), &dir, &request.call))
}
