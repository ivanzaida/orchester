//! Removing what a run left as the desktop asks it (*Remove worktree*, a merge's clean-up, deleting a task, the MCP
//! `remove_worktree`; [`orchester_ipc::Command::RemoveWorktrees`]): the desktop removes no run's worktree, stops no
//! process in it and prunes no index itself (`docs/architecture/remote-runners.md`, rule 5). It records the removal in
//! the project's database before and after; this removes ([`removal::serve`]), stopping the run's own processes in a
//! folder something held when asked, then prunes the codebase indexes and sweeps the worktree folders.

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::removal::{self, RemovalFailure, RemovalRequest},
  run::Run,
  store::RunStore,
};
use orchester_exec::{ProcessRunner, run_processes::RunProcesses};
use orchester_ipc::Reply;
use orchester_store::ProjectDb;

use super::removal::{REMOVE_TIMEOUT, after_removal};
use crate::{execution::Scheduler, execution_root::ExecutionRoot};

impl Scheduler {
  /// The answer to [`orchester_ipc::Command::RemoveWorktrees`] for `run`; git runs off the runner's async workers.
  pub async fn remove_worktrees(self, run: RunId, request: RemovalRequest) -> Reply {
    let execution = ExecutionRoot::local(&self.inner.project_root);
    let removed = tokio::task::spawn_blocking(move || remove(&execution, run, &request))
      .await
      .unwrap_or_else(|error| {
        tracing::error!(%error, %run, "removing the run's worktrees ended unexpectedly");
        Err(RemovalFailure::GitStopped("the removal ended unexpectedly".to_owned()))
      });
    match removed {
      Ok(cleanup) => Reply::WorktreesRemoved(cleanup),
      Err(failure) => Reply::RemovalFailed(failure),
    }
  }
}

/// Removes what `run` left in the repository of `execution` as `request` allows, then what follows a removal.
fn remove(
  execution: &ExecutionRoot,
  run: RunId,
  request: &RemovalRequest,
) -> Result<orchester_core::integration::Cleanup, RemovalFailure> {
  let root = execution.repository();
  let run = read_run(root, run)?;
  let stop = |folder: &Path| RunProcesses::stop_in(folder, run.created_at());
  let git = ProcessRunner::new(REMOVE_TIMEOUT);
  let removed = removal::serve(&git, (Path::new("git"), root), &run, request, stop);
  match &removed {
    Ok(cleanup) => {
      if let Some(left) = cleanup.not_removed() {
        tracing::warn!(run = %run.id(), path = %left.path, reason = ?left.reason, "a worktree of the run was not removed");
      }
      after_removal(root);
    }
    Err(failure) => tracing::warn!(run = %run.id(), %failure, "cannot remove what the run left"),
  }
  removed
}

/// Run `run` as the project's database at `root` holds it, read without writing.
fn read_run(root: &Path, run: RunId) -> Result<Run, RemovalFailure> {
  let unreadable = |detail: String| {
    tracing::warn!(%run, %detail, "cannot read the run whose worktrees are to go");
    RemovalFailure::RunUnreadable(detail)
  };
  let db = ProjectDb::open_read_only(root).map_err(|error| unreadable(error.to_string()))?;
  db.run(run)
    .map_err(|error| unreadable(error.to_string()))?
    .ok_or_else(|| unreadable("the project has no such run".to_owned()))
}
