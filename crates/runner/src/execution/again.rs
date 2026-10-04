//! Run again: whether a task whose latest run failed or was cancelled may start its next run
//! (the rule is core's, [`run_again`]), and removing that run's worktrees first when the person
//! asked to, with the same rules as *Remove worktree* ([`clean_up`]): a worktree with uncommitted
//! changes goes only with exactly the changes the person saw. The codebase indexes only removed
//! worktrees used go with them. An interrupted run whose worktree a person removed no longer holds
//! its task (ORC-63), and removing a run's own worktree is recorded.

use std::path::Path;

use orchester_core::{
  id::{RunId, TaskId},
  integration::{KeepReason, Leftovers, clean_up},
  resource::Pipeline,
  run::{Run, RunAgainInput, run_again},
  store::{ActivityStore, ResourceStore, RunStore, SettingsStore, TaskStore, WorktreeStore},
  time::Timestamp,
};
use orchester_exec::ProcessRunner;
use orchester_index::CodebaseIndexes;
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{failed, read_failed};

/// Git removing a worktree full of build output gets as long as the desktop gives it.
const REMOVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// The run `task` runs again after, `after`, once core's rule allows it.
pub(super) fn check(db: &ProjectDb, task: TaskId, after: RunId) -> Result<Run, Rejection> {
  let store = |error: &dyn std::error::Error| failed("cannot read the project database", error);
  let task = db
    .task(task)
    .map_err(|error| store(&error))?
    .ok_or(Rejection::TaskNotFound)?;
  let listed = db
    .runs_of_task(task.id())
    .map_err(|error| read_failed("cannot read runs", error))?;
  let runs: Vec<Option<&Run>> = listed.iter().map(|run| run.as_ref().ok()).collect();
  let pipeline_published = match task.details().pipeline {
    Some(pipeline) => db
      .latest_version::<Pipeline>(pipeline)
      .map_err(|error| store(&error))?
      .is_some(),
    None => false,
  };
  let limits = db.project_limits().map_err(|error| store(&error))?;
  let used_today = db
    .tokens_used_since(Timestamp::now().local_day_start())
    .map_err(|error| store(&error))?;
  let worktree_removed: Vec<RunId> = db
    .worktree_removals(task.id())
    .map_err(|error| store(&error))?
    .into_iter()
    .map(|(run, _)| run)
    .collect();
  let input = RunAgainInput {
    runs: &runs,
    after,
    worktree_removed: &worktree_removed,
    pipeline_published,
    limits,
    used_today,
  };
  run_again(&input).map_err(|block| Rejection::RunAgainBlocked { block })?;
  let previous = runs
    .last()
    .copied()
    .flatten()
    .expect("the rule allows running again only after the task's latest run");
  Ok(previous.clone())
}

/// Removes what `previous` left in the repository at `root`, as the person confirmed it
/// (`confirmed`), and records it in `db` when its own worktree went; refused when a worktree stays.
pub(super) fn remove_worktrees(
  db: &mut ProjectDb,
  root: &Path,
  previous: &Run,
  confirmed: &Leftovers,
) -> Result<(), Rejection> {
  let git = ProcessRunner::new(REMOVE_TIMEOUT);
  let cleanup = clean_up(&git, Path::new("git"), root, previous, Some(confirmed))
    .map_err(|error| failed("cannot remove the worktrees of the run before", &error))?;
  if cleanup.removed_run_worktree() {
    db.save_worktree_removal(previous.id(), Timestamp::now())
      .map_err(|error| failed("cannot record that the run's worktree was removed", &error))?;
  }
  // Off the runtime's threads: the worktrees are gone either way, and an index left behind goes
  // with the next removal.
  let indexes = CodebaseIndexes::new(root);
  tokio::task::spawn_blocking(move || {
    if let Err(error) = indexes.prune() {
      tracing::warn!(%error, "cannot remove the codebase indexes of removed worktrees");
    }
  });
  match cleanup.kept_worktrees.into_iter().next() {
    None => Ok(()),
    Some((worktree, KeepReason::Failed(detail))) => {
      tracing::warn!(path = %worktree.path, %detail, "git did not remove the worktree of the run before");
      Err(Rejection::WorktreeNotRemoved {
        path: worktree.path,
        detail,
      })
    }
    Some((worktree, _)) => Err(Rejection::WorktreeChanged { path: worktree.path }),
  }
}
