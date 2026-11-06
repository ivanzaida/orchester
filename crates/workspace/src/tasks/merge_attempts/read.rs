//! What Review › Merges and the attempt page read of the project's merges (`crates/desktop-app/design/orchester.pen`,
//! board `Review · redesign`, 09 to 09l): the runs whose last attempt did not land, the runs that wait for a person's
//! review before they merge, the merged runs by day, and one run's attempts. Blocks on SQLite and git.

use std::path::Path;

use orchester_core::{
  id::{RunId, TaskId},
  integration::{LoggedCommit, RunMerge, attempt::RecordedAttempt, branch_head, own_commits},
  merge_queue::{QueueEntry, QueueState},
  revision::Revision,
  run::RunNumber,
  store::RunStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::{
  changes::GIT,
  data::ProjectTasks,
  detail::run_store_error,
  needs_you::{WaitingKind, WaitingTask},
};

/// A run whose last merge attempt did not land, and that was not merged since.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotMerged {
  pub attempt: RecordedAttempt,
  /// Whether Orchester tries the merge again by itself; `None` when it waits for a person.
  pub retrying: Option<Retrying>,
}

/// Orchester tries a merge again by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retrying {
  /// When it tries next; `None` when the merge queue tries once it gets to the run, at no set time.
  pub next: Option<Timestamp>,
}

/// The runs of the project at `root` whose last merge attempt did not land and that were not merged since, newest
/// first, each with whether Orchester tries again: its own retry of a cause that passes, at a set time
/// ([`crate::tasks::merge_retry`]), or the merge queue's. A run the merge queue took back and that waits for a person's
/// approval there is not among them: it waits for review ([`waiting_for_review`]).
pub fn not_merged(root: &Path) -> Result<Vec<NotMerged>, StoreError> {
  let db = ProjectDb::open(root)?;
  let mut runs = Vec::new();
  for attempt in db.unlanded_merge_attempts()? {
    let run = attempt.attempt.run();
    if awaits_approval(&db, run)? {
      continue;
    }
    let retrying = match crate::tasks::merge_retry::waiting(run) {
      Some(wait) => Some(Retrying { next: Some(wait.at) }),
      None => queue_retries(&db, run)?.then_some(Retrying { next: None }),
    };
    runs.push(NotMerged { attempt, retrying });
  }
  Ok(runs)
}

/// Whether `run` waits for a person's approval in the merge queue.
fn awaits_approval(db: &ProjectDb, run: RunId) -> Result<bool, StoreError> {
  Ok(
    db.merge_queue_entry(run)?
      .is_some_and(|entry| entry.state == QueueState::WaitingForApproval),
  )
}

/// Whether the merge queue delivers `run` on its own: it is in the queue, and nothing it waits for is a person's.
pub(super) fn queue_retries(db: &ProjectDb, run: RunId) -> Result<bool, StoreError> {
  Ok(db.merge_queue_entry(run)?.is_some_and(|entry| {
    matches!(
      entry.state,
      QueueState::Queued
        | QueueState::Syncing
        | QueueState::Rechecking { .. }
        | QueueState::Integrating { .. }
        | QueueState::Blocked(_)
    )
  }))
}

/// Where a run waits for a person's review before it merges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewIn {
  /// *Ready for review* on its task page.
  TaskPage,
  /// `WaitingForApproval` in the merge queue.
  MergeQueue,
}

/// A run that waits for a person's review before it merges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingReview {
  pub task: WaitingTask,
  pub run: RunId,
  pub number: RunNumber,
  pub since: Timestamp,
  pub review_in: ReviewIn,
  /// Where the run's branch stands against the head that was reviewed ([`reviewed_head`]); `None` when that cannot be
  /// told.
  pub since_review: Option<SinceReview>,
}

/// Where a run's branch stands against the head that was reviewed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SinceReview {
  /// The branch is at the head reviewed.
  Unchanged,
  /// The branch moved after `reviewed`, or is gone: *Changed since your review*.
  Moved {
    reviewed: Revision,
    /// The run's commits after `reviewed` up to where its branch is, newest first ([`commits_after`]).
    commits: Vec<LoggedCommit>,
  },
}

/// The runs of `tasks`, the project at `root`'s, that wait for a person's review before they merge, newest first.
pub fn waiting_for_review(
  tasks: &ProjectTasks,
  (root, programs): (&Path, &Programs),
) -> Result<Vec<WaitingReview>, StoreError> {
  let db = ProjectDb::open(root)?;
  let mut waiting = Vec::new();
  for item in tasks.waiting().iter() {
    let (WaitingKind::ReadyToMerge, Some(task), Some((run, number))) = (&item.kind, &item.task, item.run) else {
      continue;
    };
    let entry = db.merge_queue_entry(run)?;
    let in_queue = entry
      .as_ref()
      .filter(|entry| entry.state == QueueState::WaitingForApproval);
    let reviewed = reviewed_head(&db, run, in_queue)?;
    waiting.push(WaitingReview {
      task: task.clone(),
      run,
      number,
      since: item.since,
      review_in: if in_queue.is_some() {
        ReviewIn::MergeQueue
      } else {
        ReviewIn::TaskPage
      },
      since_review: moved_from(&db, (root, programs), run, reviewed.as_ref())?,
    });
  }
  waiting.sort_by_key(|item| std::cmp::Reverse(item.since));
  Ok(waiting)
}

/// The head the person reviews `run` at, which *Approve and merge* pins (ORC-366): the tip the merge queue's last check
/// ran on while it waits there for approval (`queued`), else the run's latest checkpoint; `None` without either.
pub(crate) fn reviewed_head(
  db: &ProjectDb,
  run: RunId,
  queued: Option<&QueueEntry>,
) -> Result<Option<Revision>, StoreError> {
  if let Some(checked) = queued.and_then(|entry| entry.checked.as_ref()) {
    return Ok(Some(checked.tip.clone()));
  }
  Ok(
    db.latest_checkpoint(run)
      .map_err(run_store_error)?
      .map(|checkpoint| checkpoint.revision().clone()),
  )
}

/// When the head [`reviewed_head`] names was reached: the merge queue's last check on it, else the run's latest
/// checkpoint; `None` without either.
pub(crate) fn reviewed_at(
  db: &ProjectDb,
  run: RunId,
  queued: Option<&QueueEntry>,
) -> Result<Option<Timestamp>, StoreError> {
  if let Some(checked) = queued.and_then(|entry| entry.checked.as_ref()) {
    return Ok(Some(checked.at));
  }
  Ok(
    db.latest_checkpoint(run)
      .map_err(run_store_error)?
      .map(|checkpoint| checkpoint.created_at()),
  )
}

/// The run's commits after `reviewed` up to `head`, newest first, at most
/// [`MAX_LISTED`](orchester_core::integration::MAX_LISTED); none while they are the same, or when git cannot tell (the
/// log says why).
pub(crate) fn commits_after(
  (root, programs): (&Path, &Programs),
  (reviewed, head): (&Revision, &Revision),
) -> Vec<LoggedCommit> {
  if reviewed == head {
    return Vec::new();
  }
  own_commits(&programs.runner(), Path::new(GIT), (root, reviewed, head)).unwrap_or_else(|error| {
    tracing::warn!(%error, "cannot list the commits after the reviewed head");
    Vec::new()
  })
}

/// Where `run`'s branch stands against `reviewed`: at it, or moved past it or gone; `None` when that cannot be told,
/// with no reviewed head or a branch Git could not read (the log says why).
fn moved_from(
  db: &ProjectDb,
  (root, programs): (&Path, &Programs),
  run: RunId,
  reviewed: Option<&Revision>,
) -> Result<Option<SinceReview>, StoreError> {
  let (Some(reviewed), Some(stored)) = (reviewed, db.run(run).map_err(run_store_error)?) else {
    return Ok(None);
  };
  match branch_head(&programs.runner(), Path::new(GIT), (root, stored.branch())) {
    Ok(Some(head)) if head == *reviewed => Ok(Some(SinceReview::Unchanged)),
    Ok(head) => Ok(Some(SinceReview::Moved {
      commits: head.map_or_else(Vec::new, |head| commits_after((root, programs), (reviewed, &head))),
      reviewed: reviewed.clone(),
    })),
    Err(error) => {
      tracing::warn!(%error, %run, "cannot read where the run's branch is");
      Ok(None)
    }
  }
}

/// The runs merged on one local day, newest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedDay {
  /// Local midnight of the day.
  pub day: Timestamp,
  pub runs: Vec<(TaskId, RunMerge)>,
}

/// A page of merged runs: by local day, newest first, and where the next page starts, when there is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedPage {
  pub days: Vec<MergedDay>,
  pub next: Option<(Timestamp, RunId)>,
}

/// At most `limit` of the runs Orchester merged in the project at `root`, those merged before `after` (the page's
/// `next` before), grouped by local day, newest first.
pub fn merged_by_day(root: &Path, after: Option<(Timestamp, RunId)>, limit: usize) -> Result<MergedPage, StoreError> {
  let mut merged = ProjectDb::open(root)?.merged_runs(after, limit.saturating_add(1))?;
  let next = (merged.len() > limit).then(|| {
    merged.truncate(limit);
    merged.last().map(|(_, merge)| (merge.merged_at, merge.run))
  });
  let mut days: Vec<MergedDay> = Vec::new();
  for (task, merge) in merged {
    let day = merge.merged_at.local_day_start();
    match days.last_mut() {
      Some(last) if last.day == day => last.runs.push((task, merge)),
      _ => days.push(MergedDay {
        day,
        runs: vec![(task, merge)],
      }),
    }
  }
  Ok(MergedPage {
    days,
    next: next.flatten(),
  })
}

/// `run`'s merge attempts in the project at `root`, newest first, each with its stages.
pub fn attempts_of(root: &Path, run: RunId) -> Result<Vec<RecordedAttempt>, StoreError> {
  ProjectDb::open(root)?.merge_attempts(run)
}
