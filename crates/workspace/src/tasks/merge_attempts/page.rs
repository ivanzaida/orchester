//! What a run's attempt page shows (ORC-363; `crates/desktop-app/design/orchester.pen`, board `Review · redesign`, 09b
//! to 09l): the run as its task names it, its branch, the head that was reviewed, the branch it merges into and where
//! that is now, how the checkout stands, its attempts newest first with Orchester's retries among them, and how its
//! last one left it: merged, tried again by Orchester, or with files its bring-in left changed. Blocks on SQLite and
//! git.

use std::path::Path;

use orchester_core::{
  integration::{
    RunMerge,
    attempt::{AttemptOutcome, RecordedAttempt},
    branch_head,
  },
  merge_queue::{Left, QueueState},
  project::TaskKey,
  revision::Revision,
  run::{BranchName, Run, RunNumber},
  store::{MergeStore, ResourceStore, RunStore},
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::{
  merges::MergeRun,
  read::{Retrying, queue_retries},
};
use crate::{
  merge_queue::{BaseState, checkout_base},
  tasks::{changes::GIT, data::ProjectTasks, detail::run_store_error},
};

/// One run's merge attempts and what its page says beside them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptPage {
  pub run: MergeRun,
  pub branch: BranchName,
  /// The run's pipeline's name, as the project names it now, and the version the run pinned; no name once the
  /// pipeline is gone.
  pub pipeline: (Option<String>, u32),
  /// The head that was reviewed: the run's last checkpoint.
  pub reviewed: Option<Revision>,
  /// Where the branch the run merges into is now; `None` without one, or when git cannot tell.
  pub into_head: Option<Revision>,
  /// The branch the checkout has, with how it stands.
  pub checkout: Option<BaseState>,
  /// Newest first.
  pub attempts: Vec<RecordedAttempt>,
  /// The run's merge, once recorded.
  pub merge: Option<RunMerge>,
  /// Whether Orchester tries the merge again by itself, while its last attempt did not land.
  pub retrying: Option<Retrying>,
  /// The merge queue stopped delivering the run until someone acts on what stopped it: *Try again* puts it back in
  /// its place there (`crate::merge_queue::retry_stopped`) rather than merging it from its task page.
  pub queue_stopped: bool,
  /// The merge queue took the run back after its last attempt and waits for a person's approval there: it merges from
  /// the queue, not from its task page.
  pub awaiting_approval: bool,
  /// The files the last attempt's bring-in left changed in the checkout and that are not put back yet.
  pub left_changed: Vec<String>,
}

impl AttemptPage {
  /// The attempt the page is about: the last one.
  pub fn last(&self) -> Option<&RecordedAttempt> {
    self.attempts.first()
  }
}

/// The page of run `number` of the task `key` among `tasks`, the project at `root`'s; `None` when there is no such task
/// or run. `programs` read the checkout and the branches.
pub fn attempt_page(
  tasks: &ProjectTasks,
  (root, programs): (&Path, &Programs),
  (key, number): (&TaskKey, RunNumber),
) -> Result<Option<AttemptPage>, StoreError> {
  let Some(task) = tasks
    .tasks
    .iter()
    .chain(tasks.not_shared.iter())
    .find(|task| task.key() == key)
  else {
    return Ok(None);
  };
  let db = ProjectDb::open(root)?;
  let Some(run) = db
    .runs_of_task(task.id())
    .map_err(run_store_error)?
    .into_iter()
    .filter_map(Result::ok)
    .find(|run| run.number() == number)
  else {
    return Ok(None);
  };
  let attempts = db.merge_attempts(run.id())?;
  let merge = db.run_merge(run.id())?;
  let last = attempts
    .first()
    .map(|last| last.attempt.outcome())
    .filter(|outcome| merge.is_none() && !outcome.landed());
  let retrying = match last {
    Some(_) => retrying(&db, &run)?,
    None => None,
  };
  let queue = match last {
    Some(_) => db.merge_queue_entry(run.id())?.map(|entry| entry.state),
    None => None,
  };
  let queue_stopped = matches!(queue, Some(QueueState::Left(Left::Stopped { .. })));
  let awaiting_approval = queue == Some(QueueState::WaitingForApproval);
  let left_changed = left_changed((root, programs), &run, last);
  Ok(Some(AttemptPage {
    run: MergeRun {
      task: task.id(),
      key: task.key().clone(),
      title: task.details().title.as_str().into(),
      run: run.id(),
      number,
      base: run.base_branch().cloned(),
    },
    branch: run.branch().clone(),
    pipeline: pipeline(&db, &run)?,
    reviewed: run.last_checkpoint().map(|checkpoint| checkpoint.revision().clone()),
    into_head: into_head((root, programs), &run),
    checkout: checkout_base(root, programs),
    attempts,
    merge,
    retrying,
    queue_stopped,
    awaiting_approval,
    left_changed,
  }))
}

/// The files the bring-in of `run`'s `last` attempt, which did not land, left changed and that are not put back yet;
/// none after another outcome, or when git cannot tell (the log says why).
fn left_changed((root, programs): (&Path, &Programs), run: &Run, last: Option<&AttemptOutcome>) -> Vec<String> {
  if !matches!(
    last,
    Some(AttemptOutcome::LeftChanged { .. } | AttemptOutcome::Interrupted { .. })
  ) {
    return Vec::new();
  }
  crate::tasks::merge_left::left_changed(root, programs, run.id()).unwrap_or_else(|problem| {
    tracing::warn!(?problem, run = %run.id(), "cannot tell which files a merge left changed in the checkout");
    Vec::new()
  })
}

/// Whether Orchester tries `run`'s merge again: its own retry at a set time, or the merge queue's.
fn retrying(db: &ProjectDb, run: &Run) -> Result<Option<Retrying>, StoreError> {
  if let Some(wait) = crate::tasks::merge_retry::waiting(run.id()) {
    return Ok(Some(Retrying { next: Some(wait.at) }));
  }
  Ok(queue_retries(db, run.id())?.then_some(Retrying { next: None }))
}

/// The name of `run`'s pipeline and the version it pinned.
fn pipeline(db: &ProjectDb, run: &Run) -> Result<(Option<String>, u32), StoreError> {
  let pinned = run.snapshot().pipeline();
  let name = db
    .resource(pinned.resource)?
    .map(|resource| resource.name().as_str().to_owned());
  Ok((name, pinned.number.get()))
}

/// Where the branch `run` merges into is now.
fn into_head((root, programs): (&Path, &Programs), run: &Run) -> Option<Revision> {
  let base = run.base_branch()?;
  branch_head(&programs.runner(), Path::new(GIT), (root, base)).unwrap_or_else(|error| {
    tracing::warn!(%error, run = %run.id(), "cannot read where the run's base branch is");
    None
  })
}
