//! What Review › Merges shows of the project's merges (`crates/desktop-app/design/orchester.pen`, board `Review ·
//! redesign`, 09 `Os7b3`): each run [`super::read`] finds, with its task's key and title, its number and the branch it
//! merges into; a run waiting for review with what it changes and the review comments not sent yet; a merged run with
//! who asked for the merge that landed, the attempts before it, and whether its worktree was removed and its branch
//! deleted. Blocks on SQLite and git.

use std::{
  collections::{HashMap, HashSet},
  path::Path,
  sync::Arc,
};

use orchester_core::{
  command::CommandRunner,
  diff::Changes,
  id::{RunId, TaskId},
  integration::{
    Branches, RunMerge,
    attempt::{AttemptOutcome, MergeAsker},
  },
  project::TaskKey,
  run::{BranchName, RunNumber},
  store::{RunStore, WorktreeStore},
  task::Task,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::read::{self, NotMerged, WaitingReview};
use crate::{
  merge_queue::{BaseState, checkout_base},
  tasks::{
    changes::{Checkpointed, GIT, RunRange, run_changes_in},
    data::ProjectTasks,
    detail::run_store_error,
    needs_you::WaitingKind,
  },
};

/// A run as a row of Review › Merges names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeRun {
  pub task: TaskId,
  pub key: TaskKey,
  pub title: Arc<str>,
  pub run: RunId,
  pub number: RunNumber,
  /// The branch the run merges into; `None` when the run did not record it.
  pub base: Option<BranchName>,
}

/// A run whose last merge attempt did not land.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotMergedRun {
  pub run: MergeRun,
  pub not_merged: NotMerged,
  /// The files its merge's bring-in left changed in the checkout and that are not put back yet
  /// ([`crate::tasks::merge_left`]); read only after an attempt that stopped or ended while it brought the merge in.
  pub left_changed: Vec<String>,
}

/// A run that waits for a person's review before it merges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingRun {
  pub run: MergeRun,
  pub waiting: WaitingReview,
  /// What the run changes up to its latest checkpoint; `None` when it cannot be read (the log says why).
  pub size: Option<Changes>,
  /// The review comments written and not sent to Implement yet.
  pub unsent: usize,
}

/// A run Orchester merged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedRun {
  pub run: MergeRun,
  pub merge: RunMerge,
  /// Who asked for the merge that landed; `None` for a merge recorded before attempts were.
  pub asked_by: Option<MergeAsker>,
  /// The attempts before the one that landed, which did not.
  pub refused_before: usize,
  pub worktree_removed: bool,
  /// The run's branch is no longer in the repository; `false` when git could not tell (the log says why).
  pub branch_deleted: bool,
}

/// The runs merged on one local day, newest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedRunsDay {
  /// Local midnight of the day.
  pub day: Timestamp,
  pub runs: Vec<MergedRun>,
}

/// Every section of Review › Merges, newest first in each, and the branch the checkout has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merges {
  /// The branch the checkout has, with how it stands; `None` on a detached HEAD or when git cannot tell.
  pub base: Option<BaseState>,
  pub not_merged: Vec<NotMergedRun>,
  /// The runs waiting for review that are not among `not_merged`.
  pub waiting: Vec<WaitingRun>,
  pub merged: Vec<MergedRunsDay>,
  /// More runs were merged before the last one listed.
  pub more: bool,
}

impl Merges {
  /// The runs that need the person here: not merged, and waiting for their review.
  pub fn needing_you(&self) -> usize {
    self.not_merged.len() + self.waiting.len()
  }

  /// Nothing was ever tried: no section has a row.
  pub fn is_empty(&self) -> bool {
    self.not_merged.is_empty() && self.waiting.is_empty() && self.merged.is_empty()
  }
}

/// The merges of `tasks`, the project at `root`'s: the runs not merged, those waiting for review, and the `merged`
/// runs merged last, by local day. `programs` read the checkout, where the runs' branches are and what they change. A
/// run whose task is not among `tasks` is left out.
pub fn merges(tasks: &ProjectTasks, (root, programs): (&Path, &Programs), merged: usize) -> Result<Merges, StoreError> {
  let named = named(tasks);
  let db = ProjectDb::open(root)?;
  let runs = Runs { db: &db, named: &named };
  let mut not_merged = Vec::new();
  for attempt in read::not_merged(root)? {
    let (task, run) = (attempt.attempt.task, attempt.attempt.attempt.run());
    if let Some(run) = runs.of(task, run)? {
      not_merged.push(NotMergedRun {
        left_changed: left_changed((root, programs), &attempt),
        run,
        not_merged: attempt,
      });
    }
  }
  let mut waiting = Vec::new();
  for review in read::waiting_for_review(tasks, (root, programs))? {
    if not_merged.iter().any(|shown| shown.run.run == review.run) {
      continue;
    }
    if let Some(run) = runs.of(review.task.id, review.run)? {
      waiting.push(waiting_run(&db, (root, programs), run, review)?);
    }
  }
  let (merged, more) = merged_days((root, programs), &runs, merged)?;
  Ok(Merges {
    base: checkout_base(root, programs),
    not_merged,
    waiting,
    merged,
    more,
  })
}

/// How many runs of `tasks`, the project at `root`'s, need the person in Review › Merges, as [`Merges::needing_you`]
/// counts them, without git: the runs not merged, and those waiting for review that are not among them.
pub fn needing_you(tasks: &ProjectTasks, root: &Path) -> Result<usize, StoreError> {
  let named = named(tasks);
  let not_merged: HashSet<RunId> = read::not_merged(root)?
    .iter()
    .filter(|shown| named.contains_key(&shown.attempt.task))
    .map(|shown| shown.attempt.attempt.run())
    .collect();
  let waiting = tasks
    .waiting()
    .iter()
    .filter(|item| match (&item.kind, &item.task, item.run) {
      (WaitingKind::ReadyToMerge, Some(_), Some((run, _))) => !not_merged.contains(&run),
      _ => false,
    })
    .count();
  Ok(not_merged.len() + waiting)
}

/// The files the attempt's bring-in left changed and that are not put back yet, after an attempt that stopped or ended
/// while it brought the merge in; none otherwise, or when git cannot tell (the log says why).
fn left_changed((root, programs): (&Path, &Programs), attempt: &NotMerged) -> Vec<String> {
  let outcome = attempt.attempt.attempt.outcome();
  if !matches!(
    outcome,
    AttemptOutcome::LeftChanged { .. } | AttemptOutcome::Interrupted { .. }
  ) {
    return Vec::new();
  }
  let run = attempt.attempt.attempt.run();
  crate::tasks::merge_left::left_changed(root, programs, run).unwrap_or_else(|problem| {
    tracing::warn!(?problem, %run, "cannot tell which files a merge left changed in the checkout");
    Vec::new()
  })
}
/// The tasks of `tasks` by id, those kept *Not shared* too.
fn named(tasks: &ProjectTasks) -> HashMap<TaskId, &Task> {
  tasks
    .tasks
    .iter()
    .chain(tasks.not_shared.iter())
    .map(|task| (task.id(), task))
    .collect()
}

/// The `limit` runs merged last in the project at `root`, by local day, and whether more were merged before them.
/// `programs` read once which of their branches are left.
fn merged_days(
  (root, programs): (&Path, &Programs),
  runs: &Runs<'_>,
  limit: usize,
) -> Result<(Vec<MergedRunsDay>, bool), StoreError> {
  let page = read::merged_by_day(root, None, limit)?;
  let runner = programs.runner();
  let branches = if page.days.is_empty() {
    None
  } else {
    Branches::read(&runner, Path::new(GIT), root)
      .inspect_err(|error| tracing::warn!(%error, "cannot read which merged runs' branches are left"))
      .ok()
  };
  let mut days = Vec::with_capacity(page.days.len());
  for day in page.days {
    let mut shown = Vec::with_capacity(day.runs.len());
    for (task, merge) in day.runs {
      if let Some(run) = runs.of(task, merge.run)? {
        shown.push(merged_run(runs.db, branches.as_ref(), run, merge)?);
      }
    }
    if !shown.is_empty() {
      days.push(MergedRunsDay {
        day: day.day,
        runs: shown,
      });
    }
  }
  Ok((days, page.next.is_some()))
}

/// The project's runs, named by their tasks.
struct Runs<'a> {
  db: &'a ProjectDb,
  named: &'a HashMap<TaskId, &'a Task>,
}

impl Runs<'_> {
  /// `run` of `task` as a row names it; `None` when the task or the run is gone.
  fn of(&self, task: TaskId, run: RunId) -> Result<Option<MergeRun>, StoreError> {
    let Some(task) = self.named.get(&task) else {
      tracing::warn!(%task, %run, "a merge of a task the project no longer lists is left out of Review › Merges");
      return Ok(None);
    };
    let Some(stored) = self.db.run(run).map_err(run_store_error)? else {
      tracing::warn!(%run, "a merge of a run the project no longer holds is left out of Review › Merges");
      return Ok(None);
    };
    Ok(Some(MergeRun {
      task: task.id(),
      key: task.key().clone(),
      title: task.details().title.as_str().into(),
      run,
      number: stored.number(),
      base: stored.base_branch().cloned(),
    }))
  }
}

fn waiting_run(
  db: &ProjectDb,
  (root, programs): (&Path, &Programs),
  run: MergeRun,
  waiting: WaitingReview,
) -> Result<WaitingRun, StoreError> {
  let size = match db.run(run.run).map_err(run_store_error)? {
    Some(stored) => match run_changes_in(root, programs, &RunRange::of(&stored)) {
      Ok(Checkpointed::Read { changes, .. }) => Some(Changes {
        files: u32::try_from(changes.diff.files().len()).unwrap_or(u32::MAX),
        additions: changes.diff.additions(),
        deletions: changes.diff.deletions(),
      }),
      Ok(Checkpointed::TooLarge { size, .. }) => Some(size),
      Ok(Checkpointed::NoCheckpoint) => None,
      Err(error) => {
        tracing::warn!(%error, run = %run.run, "cannot read what a run waiting for review changes");
        None
      }
    },
    None => None,
  };
  let unsent = db
    .review_threads(run.run)?
    .iter()
    .filter(|thread| thread.resolved_at().is_none())
    .map(|thread| thread.unsent().count())
    .sum();
  Ok(WaitingRun {
    run,
    waiting,
    size,
    unsent,
  })
}

/// `run`, merged as `merge`; its branch is told deleted when `branches`, the repository's, no longer hold it.
fn merged_run<R: CommandRunner>(
  db: &ProjectDb,
  branches: Option<&Branches<'_, R>>,
  run: MergeRun,
  merge: RunMerge,
) -> Result<MergedRun, StoreError> {
  let attempts = db.merge_attempts(run.run)?;
  let landed = attempts.iter().position(|recorded| recorded.attempt.outcome().landed());
  let (asked_by, refused_before) = match landed {
    Some(at) => (Some(attempts[at].attempt.asked_by()), attempts.len() - at - 1),
    None => (None, 0),
  };
  let branch_deleted = match branches {
    Some(branches) => db
      .run(run.run)
      .map_err(run_store_error)?
      .is_some_and(|stored| !branches.has(stored.branch())),
    None => false,
  };
  Ok(MergedRun {
    worktree_removed: db.worktree_removal(run.run)?.is_some(),
    branch_deleted,
    run,
    merge,
    asked_by,
    refused_before,
  })
}
