//! Run again ([`Scheduler::run_again`]): whether a task whose latest run failed or was cancelled may start its next run
//! (the rule is core's, [`run_again`]), and having the executor remove that run's worktrees first when the person
//! asked to (`super::super::executor::removal`). An interrupted run whose worktree a person removed no longer holds
//! its task (ORC-63), and removing a run's own worktree is recorded. A run that passed every step and committed
//! nothing although its pipeline expects changes runs again too (ORC-93), as git tells from the previous run's
//! branches, read here. The next run is created as the first one is (`super::prepare`).

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  integration::{Branches, Delivered, Leftovers, RunWork},
  resource::{Pipeline, WithoutCommits},
  run::{Run, RunAgainInput, RunStatus, run_again, run_delivers},
  sharing::TaskRevision,
  store::{ActivityStore, ResourceStore, RunStore, SettingsStore, TaskStore, WorktreeStore},
  task::Task,
  time::Timestamp,
};
use orchester_exec::ProcessRunner;
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{Scheduler, failed, read_failed, sink::LocalSink};
use crate::execution::executor::{removal, start};

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
      let mut sink = LocalSink::new(root);
      removal::remove_worktrees((&db, &mut sink), root, &previous, confirmed)?;
    }
    // Compared again where the run is recorded: an edit after the check above is refused there.
    let prepared = super::prepare::record(&mut db, root, key, checked, revision);
    self.begin(db, task, prepared)
  }
}

/// Records the removal of the worktree of each interrupted run of `task` whose folder is gone
/// (deleted by hand), as the executor finds it: it can never continue, so it no longer holds its task. A run that
/// continued meanwhile has live work and is left alone.
pub(super) fn note_missing_worktrees(db: &mut ProjectDb, root: &Path, task: TaskId) -> Result<(), Rejection> {
  let store = |error: &dyn std::error::Error| failed("cannot record a removed worktree", error);
  let listed = db
    .runs_of_task(task)
    .map_err(|error| read_failed("cannot read runs", error))?;
  let interrupted = listed
    .iter()
    .flatten()
    .filter(|run| matches!(run.status(), RunStatus::Interrupted(_)));
  for run in interrupted {
    let recorded = db.worktree_removal(run.id()).map_err(|error| store(&error))?;
    if recorded.is_none()
      && start::own_worktree(root, run.id())
        .map_err(|error| store(&error))?
        .is_none()
    {
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
