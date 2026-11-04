//! The Orchestrator chat's git in a run's worktree, run here for the desktop ([`orchester_ipc::Command::RunGit`],
//! `docs/architecture/remote-runners.md` rule 5): the desktop never runs git there itself. The run's worktree belongs
//! to its agents until the run ended, so until then only reads run ([`RunGitRequest::allowed_before_end`]); the
//! worktree is the run's own, found here, never a folder the desktop names. Every answer fits one frame
//! ([`MOST_OUTPUT`]).

use std::{future::Future, path::Path};

use orchester_core::{
  orchestrator::run_git::{RunGitAnswer, RunGitRequest},
  store::{RunStore, WorktreeStore},
};
use orchester_exec::{programs::Programs, run_git::serve};
use orchester_ipc::{Rejection, Reply, frame::MAX_FRAME_LEN};
use orchester_store::ProjectDb;

use super::super::{Scheduler, failed};
use crate::execution_root::ExecutionRoot;

/// The most of each of git's two outputs an answer carries: a frame holds at most [`MAX_FRAME_LEN`], and JSON writes
/// a control character of the text with six bytes. Git that writes more than this where the desktop wants it all is
/// answered as not run (`orchester_exec::run_git::too_large`), never with a frame too large to send.
const MOST_OUTPUT: usize = MAX_FRAME_LEN as usize / 16;

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
/// only read, or has no worktree any more; the worktree is the run's own in the executor's execution root. Blocks on
/// SQLite and git.
fn answer(root: &Path, request: &RunGitRequest) -> Result<RunGitAnswer, Rejection> {
  let db = ProjectDb::open_read_only(root).map_err(|error| failed("cannot open the project database", &error))?;
  let run = db
    .run(request.run)
    .map_err(|error| failed("cannot read the run", &error))?
    .ok_or(Rejection::RunNotFound)?;
  if !run.status().is_terminal() && !request.allowed_before_end() {
    return Err(Rejection::RunNotEnded);
  }
  let dir = ExecutionRoot::local(root).run_worktree(run.id());
  let removed = db
    .worktree_removal(run.id())
    .map_err(|error| failed("cannot read the worktree's removal", &error))?;
  if removed.is_some() || !dir.is_dir() {
    return Err(Rejection::WorktreeRemoved);
  }
  Ok(serve(&Programs::processes(), &dir, &request.call, MOST_OUTPUT))
}
