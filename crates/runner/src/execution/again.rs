//! Run again ([`Scheduler::run_again`]): whether a task whose latest run failed or was cancelled may start its next run
//! (the rule is core's, [`run_again`]), and removing that run's worktrees first when the person
//! asked to, with the same rules as *Remove worktree* ([`clean_up`]): a worktree with uncommitted
//! changes goes only with exactly the changes the person saw. The codebase indexes only removed
//! worktrees used go with them. An interrupted run whose worktree a person removed no longer holds
//! its task (ORC-63), and removing a run's own worktree is recorded. A run that passed every step and committed
//! nothing although its pipeline expects changes runs again too (ORC-93), as git tells.

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  integration::{Branches, Delivered, KeepReason, Leftovers, RunWork, clean_up_recorded},
  resource::{Pipeline, WithoutCommits},
  run::{Run, RunAgainInput, RunStatus, run_again, run_delivers},
  sharing::TaskRevision,
  store::{ActivityStore, ResourceStore, RunStore, SettingsStore, TaskStore, WorktreeStore},
  task::Task,
  time::Timestamp,
};
use orchester_exec::ProcessRunner;
use orchester_index::CodebaseIndexes;
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{Scheduler, failed, read_failed};
use crate::worktree::Worktrees;

/// How long git may take to list the branches when it tells whether a run committed nothing.
const GIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

impl Scheduler {
  /// Starts the next run of `task` after its latest run `after` failed or was cancelled (Run
  /// again), removing `remove_worktrees`, what `after` left, first; or finds the run an earlier
  /// request with `key` started, and then removes nothing. Returns once the new run is recorded as
  /// started.
  pub async fn run_again(
    &self,
    key: IdempotencyKey,
    task: TaskId,
    after: RunId,
    remove_worktrees: Option<Leftovers>,
  ) -> Result<RunId, Rejection> {
    self.run_again_as(key, (task, after), remove_worktrees, None).await
  }

  /// [`Self::run_again`], only for the task at `revision` when it names one: a task changed since it was checked is
  /// refused ([`Rejection::TaskChangedMeanwhile`]), read with the run's own check of the task.
  pub async fn run_again_as(
    &self,
    key: IdempotencyKey,
    (task, after): (TaskId, RunId),
    remove_worktrees: Option<Leftovers>,
    revision: Option<TaskRevision>,
  ) -> Result<RunId, Rejection> {
    // The run's task may still be ending its agents after it recorded the run's end. A run its
    // task still executes has not ended, which the rule finds in the run's record below.
    if let Err(executing) = self.settled(after).await {
      tracing::debug!(%after, ?executing, "run again asked while the run is executed");
    }
    let _one_at_a_time = self.inner.continuing.lock().await;
    let root = &self.inner.project_root;
    let mut db = self.open()?;
    let recorded = db
      .run_with_key(key)
      .map_err(|error| read_failed("cannot read runs", error))?;
    if let Some(recorded) = recorded {
      if recorded.status() == &RunStatus::Pending {
        self.admit(&db)?;
      }
      let prepared = super::prepare::resume_recorded(&mut db, root, task, recorded);
      return self.begin(db, task, prepared);
    }
    note_missing_worktrees(&mut db, root, task)?;
    let previous = check(&db, root, (task, after))?;
    self.admit(&db)?;
    let checked = super::prepare::check_again(&db, root, task)?;
    if revision.is_some_and(|revision| checked.task().revision() != revision) {
      return Err(Rejection::TaskChangedMeanwhile);
    }
    if let Some(confirmed) = &remove_worktrees {
      self::remove_worktrees(&mut db, root, &previous, confirmed)?;
    }
    // Compared again where the run is recorded: an edit after the check above is refused there.
    let prepared = super::prepare::record(&mut db, root, key, checked, revision);
    self.begin(db, task, prepared)
  }
}

/// Git removing a worktree full of build output gets as long as the desktop gives it.
const REMOVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Records the removal of the worktree of each interrupted run of `task` whose folder is gone
/// (deleted by hand): it can never continue, so it no longer holds its task. A run that continued
/// meanwhile has live work and is left alone.
pub(super) fn note_missing_worktrees(db: &mut ProjectDb, root: &Path, task: TaskId) -> Result<(), Rejection> {
  let store = |error: &dyn std::error::Error| failed("cannot record a removed worktree", error);
  let worktrees = Worktrees::new(root).map_err(|error| store(&error))?;
  let listed = db
    .runs_of_task(task)
    .map_err(|error| read_failed("cannot read runs", error))?;
  let interrupted = listed
    .iter()
    .flatten()
    .filter(|run| matches!(run.status(), RunStatus::Interrupted(_)));
  for run in interrupted {
    let recorded = db.worktree_removal(run.id()).map_err(|error| store(&error))?;
    if recorded.is_none() && !worktrees.path(run.id()).is_dir() {
      tracing::warn!(run = %run.id(), "the interrupted run's worktree is gone; it can no longer continue");
      db.begin_worktree_removal(run.id(), Timestamp::now())
        .map_err(|error| store(&error))?;
    }
  }
  Ok(())
}

/// The run `task` runs again after, `after`, once core's rule allows it; git in the repository at `root` tells whether
/// `after` committed nothing.
pub(super) fn check(db: &ProjectDb, root: &Path, (task, after): (TaskId, RunId)) -> Result<Run, Rejection> {
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
  let latest = runs.last().copied().flatten().filter(|run| run.id() == after);
  let input = RunAgainInput {
    runs: &runs,
    after,
    worktree_removed: &worktree_removed,
    pipeline_published,
    limits,
    used_today,
    committed_nothing: latest.is_some_and(|run| committed_nothing(root, &task, run)),
  };
  run_again(&input).map_err(|block| Rejection::RunAgainBlocked { block })?;
  let previous = runs
    .last()
    .copied()
    .flatten()
    .expect("the rule allows running again only after the task's latest run");
  Ok(previous.clone())
}

/// Whether `run` of `task` passed every step and committed nothing although its pipeline expects changes (ORC-93), as
/// git in the repository at `root` tells; not when git cannot tell.
fn committed_nothing(root: &Path, task: &Task, run: &Run) -> bool {
  if *run.status() != RunStatus::Succeeded || run_delivers(task, run).without_commits() != WithoutCommits::Flagged {
    return false;
  }
  let git = ProcessRunner::new(GIT_TIMEOUT);
  let delivered =
    Branches::read(&git, Path::new("git"), root).and_then(|mut branches| branches.delivered(&RunWork::of(run)));
  match delivered {
    Ok(delivered) => delivered == Delivered::NothingToMerge,
    Err(error) => {
      tracing::warn!(%error, run = %run.id(), "cannot tell whether the run committed nothing; it is not run again so");
      false
    }
  }
}

/// Removes what `previous` left in the repository at `root`, as the person confirmed it
/// (`confirmed`), with the removal recorded in `db` first; refused when a worktree stays.
pub(super) fn remove_worktrees(
  db: &mut ProjectDb,
  root: &Path,
  previous: &Run,
  confirmed: &Leftovers,
) -> Result<(), Rejection> {
  let git = ProcessRunner::new(REMOVE_TIMEOUT);
  let cleanup = clean_up_recorded(
    db,
    (&git, Path::new("git"), root),
    previous,
    Some(confirmed),
    Timestamp::now(),
  )
  .map_err(|error| failed("cannot remove the worktrees of the run before", &error))?;
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
