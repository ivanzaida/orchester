//! *Waiting for you* and *With the agent*.

use orchester_core::{
  merge_queue::Left,
  review::{CommentState, ReviewThread, Said},
  run::{Run, TaskPhase},
  time::Timestamp,
};
use orchester_store::StoreError;

use super::{
  ReviewRow, ReviewState,
  rows::{Reader, open},
};
use crate::tasks::{
  list::Latest,
  merge_attempts::read::{self, SinceReview, WaitingReview},
  needs_you::WaitingKind,
  return_work,
  review_threads::conflicts::{Standing, standing},
};

/// The runs that wait for the person, oldest wait first: those that passed every step and wait for the approval, with
/// the state of their review; those whose last merge attempt found conflicts, and those the merge queue took out with
/// conflicts, which *Conflicts with <base>* says.
pub(super) fn waiting(reader: &Reader<'_>) -> Result<Vec<ReviewRow>, StoreError> {
  let mut rows = Vec::new();
  for review in read::waiting_for_review(reader.tasks, (reader.root, reader.programs))? {
    let Some(task) = reader.task(review.task.id) else {
      continue;
    };
    let stored = reader.run(review.run)?;
    let threads = reader.db.review_threads(review.run)?;
    let conflicts = match stored.as_ref() {
      Some(run) => standing(&reader.db, run)?,
      None => None,
    };
    let state = match conflicts {
      Some(Standing { at, files }) => (
        ReviewState::Conflicts {
          base: stored.as_ref().and_then(|run| run.base_branch().cloned()),
          files: files.len(),
        },
        at,
      ),
      None => (waiting_state(&review, &threads), review.since),
    };
    rows.push(reader.row((task, (review.run, review.number)), stored.as_ref(), &threads, state));
  }
  for item in reader.tasks.waiting().iter() {
    let (WaitingKind::DeliveryLeft(Left::Conflicts { files }), Some(waiting), Some((run, number))) =
      (&item.kind, &item.task, item.run)
    else {
      continue;
    };
    let Some(task) = reader.task(waiting.id) else {
      continue;
    };
    let stored = reader.run(run)?;
    let state = ReviewState::Conflicts {
      base: stored.as_ref().and_then(|run| run.base_branch().cloned()),
      files: files.len(),
    };
    let threads = reader.db.review_threads(run)?;
    rows.push(reader.row((task, (run, number)), stored.as_ref(), &threads, (state, item.since)));
  }
  rows.sort_by_key(|row| row.since);
  Ok(rows)
}

/// The runs whose Implement works on the comments sent to it from the review page, oldest first: a run that has not
/// ended and whose threads *Resolve* did not fold has a comment sent.
pub(super) fn with_agent(reader: &Reader<'_>) -> Result<Vec<ReviewRow>, StoreError> {
  let mut rows = Vec::new();
  for (id, latest) in reader.tasks.phases.iter() {
    let Latest::Run(latest) = latest else {
      continue;
    };
    let works = matches!(
      latest.phase,
      TaskPhase::Running
        | TaskPhase::Waiting
        | TaskPhase::Stuck
        | TaskPhase::Paused
        | TaskPhase::StoppedAtLimit
        | TaskPhase::Interrupted
    );
    let Some(task) = reader
      .task(*id)
      .filter(|task| works && !task.organization().status().is_closed())
    else {
      continue;
    };
    let threads = reader.db.review_threads(latest.id)?;
    let stored = reader.run(latest.id)?;
    let sent: Vec<Timestamp> = open(&threads).filter_map(sent_at).collect();
    let standing = match sent.iter().min().copied() {
      Some(since) => Some((ReviewState::ImplementOn { comments: sent.len() }, since)),
      None => resolving(reader, stored.as_ref())?,
    };
    let Some((state, since)) = standing else {
      continue;
    };
    rows.push(reader.row(
      (task, (latest.id, latest.number)),
      stored.as_ref(),
      &threads,
      (state, since),
    ));
  }
  rows.sort_by_key(|row| (row.since, row.run));
  Ok(rows)
}

/// The run Implement works on to resolve the conflicts of its merge, and since when: the merge attempt that found them.
fn resolving(reader: &Reader<'_>, run: Option<&Run>) -> Result<Option<(ReviewState, Timestamp)>, StoreError> {
  let Some(run) = run.filter(|run| return_work::resolving_conflicts(run).is_some()) else {
    return Ok(None);
  };
  let found = reader
    .db
    .merge_attempts(run.id())?
    .first()
    .map(|last| last.attempt.at());
  Ok(found.map(|at| {
    let state = ReviewState::ResolvingConflicts {
      base: run.base_branch().cloned(),
    };
    (state, at)
  }))
}

/// When `thread`'s comment was sent to Implement, which has not addressed it yet.
fn sent_at(thread: &ReviewThread) -> Option<Timestamp> {
  match thread.sent()?.said() {
    Said::Comment(CommentState::Sent { at }) => Some(*at),
    _ => None,
  }
}

/// Where a run that waits for the approval stands in its review: comments waiting to be sent, a branch that moved after
/// the head that was reviewed, else ready.
fn waiting_state(review: &WaitingReview, threads: &[ReviewThread]) -> ReviewState {
  let count: usize = open(threads)
    .map(|thread| {
      thread
        .messages()
        .iter()
        .filter(|message| {
          matches!(
            message.said(),
            Said::Comment(CommentState::NotSent { .. } | CommentState::Sending { .. })
          )
        })
        .count()
    })
    .sum();
  if count > 0 {
    return ReviewState::CommentsNotSent { count };
  }
  match &review.since_review {
    Some(SinceReview::Moved { commits, .. }) => ReviewState::ChangedSinceReview { commits: commits.len() },
    Some(SinceReview::Unchanged) | None => ReviewState::Ready,
  }
}
