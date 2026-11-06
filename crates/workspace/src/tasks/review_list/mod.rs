//! What the Review page lists (module Review v2; design `Review - v2`, `crates/desktop-app/design/orchester.pen`): the
//! project's tasks whose run finished, in three groups. *Waiting for you*: runs that passed every step and merge only
//! once a person approves, and approved runs whose merge into their base found conflicts and merged nothing, oldest
//! wait first. *With the agent*: runs sent back from the review page whose Implement is working on the comments sent.
//! *Approved today*: runs a person approved today, merged or merging, newest first. Built from what
//! [`super::review_threads`] and [`super::merge_attempts`] read. Blocks on SQLite and git; the UI calls it off the UI
//! thread. [`waiting_count`] reads no store and no git.

mod approved;
mod rows;
mod waiting;

use std::{path::Path, sync::Arc};

use orchester_core::{
  diff::Changes,
  id::{RunId, TaskId},
  merge_queue::Left,
  project::TaskKey,
  run::{BranchName, RunNumber},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::StoreError;

use self::rows::Reader;
use super::{data::ProjectTasks, needs_you::WaitingKind};

/// Where a task's run stands in the Review list: what its row says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewState {
  /// *N comments not sent*: comments written on the run's diff wait in the tray (or are on their way) for *Send to
  /// Implement*.
  CommentsNotSent { count: usize },
  /// *Changed since your review*: the run's branch has `commits` commits after the head that was reviewed (at most
  /// [`MAX_LISTED`](orchester_core::integration::MAX_LISTED); a branch that is gone reads 0).
  ChangedSinceReview { commits: usize },
  /// *Ready for review*.
  Ready,
  /// *Conflicts with <base>*: the approved merge into `base` (`None`: the run recorded none) found `files` conflicting
  /// files and merged nothing.
  Conflicts { base: Option<BranchName>, files: usize },
  /// *Implement is on N comments*: `comments` comments sent from the review page are being worked on.
  ImplementOn { comments: usize },
  /// *Implement is resolving the conflicts with <base>*: the run went back to Implement to resolve the conflicts of its
  /// approved merge into `base`.
  ResolvingConflicts { base: Option<BranchName> },
  /// *Merged into <base>*.
  Merged { base: Option<BranchName> },
  /// Approved, and the merge queue delivers it into `base` in its turn.
  Merging { base: Option<BranchName> },
}

/// One row of the Review list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewRow {
  pub task: TaskId,
  pub key: TaskKey,
  pub title: Arc<str>,
  /// The name of the task's first tag.
  pub tag: Option<Arc<str>>,
  pub run: RunId,
  pub number: RunNumber,
  pub state: ReviewState,
  /// The run's comment threads *Resolve* did not fold. A thread a later commit outdated counts until it is resolved:
  /// telling it needs git per thread.
  pub open_threads: usize,
  /// What the run changes up to its latest checkpoint; `None` when it has no checkpoint or cannot be read (the log
  /// says why).
  pub size: Option<Changes>,
  /// When the task came to its group: how long a run waits for the person, since when Implement works on its
  /// comments, when the approval was given.
  pub since: Timestamp,
}

/// The Review list: three groups of rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewList {
  /// Oldest wait first.
  pub waiting: Vec<ReviewRow>,
  /// Oldest first.
  pub with_agent: Vec<ReviewRow>,
  /// Newest first.
  pub approved_today: Vec<ReviewRow>,
}

/// The Review list of `tasks`, the project at `root`'s, as it stands at `now`, whose local day *Approved today* is.
/// `programs` read the runs' branches and diffs.
pub fn review_list(
  tasks: &ProjectTasks,
  (root, programs): (&Path, &Programs),
  now: Timestamp,
) -> Result<ReviewList, StoreError> {
  let reader = Reader::open(tasks, (root, programs))?;
  Ok(ReviewList {
    waiting: waiting::waiting(&reader)?,
    with_agent: waiting::with_agent(&reader)?,
    approved_today: approved::approved_today(&reader, now)?,
  })
}

/// How many tasks the Review list's *Waiting for you* lists, which the sidebar shows and the list's reading state draws
/// as bones, worked out from `tasks` alone: the runs that wait for a person's approval, and the approved ones whose
/// delivery left with conflicts. A run whose merge into its base found conflicts is among the first until a person
/// acts, so the count is the group's length.
pub fn waiting_count(tasks: &ProjectTasks) -> usize {
  tasks.waiting().iter().filter(|item| is_waiting(item)).count()
}

/// Whether `item` is a run whose task is in *Waiting for you*.
fn is_waiting(item: &super::needs_you::WaitingItem) -> bool {
  item.task.is_some()
    && item.run.is_some()
    && matches!(
      item.kind,
      WaitingKind::ReadyToMerge | WaitingKind::DeliveryLeft(Left::Conflicts { .. })
    )
}
