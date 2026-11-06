//! A person's review of a run that waits for it before it merges, as the review page (ORC-367; design 09i `wLyFt`,
//! the thread and tray states of `RBR9O`) and automation read and change it (`orchester_core::review`): threads
//! anchored to lines of the run's diff at the head the person read, *Viewed* marks per head, questions that go to
//! Implement at once ([`ask`]), comments that wait unsent until *Send to Implement* ([`send`]), and *Approve and merge*
//! at the reviewed head ([`approve`]). Reading the threads recovers hand-offs to the runner cut off part-way
//! (`recover`), settles the comments Implement addressed and tells which threads a later head made outdated
//! ([`read`]). Blocks on SQLite and git.

pub mod approve;
pub mod ask;
pub mod page;
pub mod read;
mod recover;
pub mod send;
pub mod since;
mod words;

use std::path::Path;

use orchester_core::{
  conversation::MessageBody,
  id::{ReviewMessageId, ReviewThreadId, RunId},
  integration::MergeError,
  pull_request::feedback::LineAnchor,
  review::{CommentState, ReviewError, ReviewFailure, ReviewThread, Said, ThreadKind},
  revision::Revision,
  run::{Run, RunStatus},
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

pub use self::words::failure_message;
use super::{detail::run_store_error, integration::IntegrationProblem, return_work::ReturnRefusal};
use crate::{merge_queue::QueueRefusal, messages::Message};

/// Why a change of a run's review was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewRefusal {
  /// There is no such run, thread or message.
  NotFound,
  /// The run does not wait for review: it works, failed, or was merged.
  NotWaiting,
  /// Implement works on comments sent to it: nothing is approved until its work is reviewed.
  ImplementAtWork,
  /// The run's branch moved after the head the person reviewed: nothing merged, and it is reviewed again.
  ChangedSinceReview,
  /// No comment waits to be sent.
  NothingToSend,
  /// The run has no step whose agent may write: there is no Implement to ask or send to.
  NoImplementingStep,
  /// The thread does not take that change now.
  Review(ReviewError),
  /// The question was kept *Not asked*: the runner did not take it, as the failure says.
  NotAsked(ReviewFailure),
  /// The comments wait unsent again: the runner did not take them, as the failure says.
  NotSent(ReviewFailure),
  /// The run's work cannot go back to its implementing step now.
  Returned(ReturnRefusal),
  /// The merge did not happen, as the task page's merge says.
  Merge(IntegrationProblem),
  /// The merge queue did not take the approval.
  Queue(QueueRefusal),
  /// The project's data could not be read or written.
  Store(Message),
}

impl ReviewRefusal {
  /// The code automation reads.
  pub const fn code(&self) -> &'static str {
    match self {
      Self::NotFound => "not_found",
      Self::NotWaiting => "not_waiting_for_review",
      Self::ImplementAtWork => "implement_at_work",
      Self::ChangedSinceReview => "changed_since_review",
      Self::NothingToSend => "nothing_to_send",
      Self::NoImplementingStep => "no_implementing_step",
      Self::Review(_) => "not_now",
      Self::NotAsked(_) => "not_asked",
      Self::NotSent(_) => "not_sent",
      Self::Returned(refusal) => refusal.code(),
      Self::Merge(_) => "merge_failed",
      Self::Queue(refusal) => refusal.code(),
      Self::Store(_) => "store",
    }
  }

  /// What a person reads.
  pub fn message(&self) -> Message {
    match self {
      Self::NotFound => Message::new("review.refused.notFound"),
      Self::NotWaiting => Message::new("review.refused.notWaiting"),
      Self::ImplementAtWork => Message::new("review.refused.implementAtWork"),
      Self::ChangedSinceReview => Message::new("review.refused.changedSinceReview"),
      Self::NothingToSend => Message::new("review.refused.nothingToSend"),
      Self::NoImplementingStep => Message::new("review.refused.noImplementingStep"),
      Self::Review(error) => words::review_error(*error),
      Self::NotAsked(failure) | Self::NotSent(failure) => failure_message(failure),
      Self::Returned(refusal) => refusal.message(),
      Self::Merge(problem) => problem.message(", "),
      Self::Queue(refusal) => refusal.message(),
      Self::Store(message) => message.clone(),
    }
  }
}

impl From<StoreError> for ReviewRefusal {
  fn from(error: StoreError) -> Self {
    Self::Store(crate::messages::store_error(&error))
  }
}

impl From<IntegrationProblem> for ReviewRefusal {
  fn from(problem: IntegrationProblem) -> Self {
    match problem {
      IntegrationProblem::Merge(MergeError::BranchMoved { .. }) => Self::ChangedSinceReview,
      other => Self::Merge(other),
    }
  }
}

/// `run` of the project in `db`, which must wait for review: it succeeded and was not merged.
fn waiting(db: &ProjectDb, run: RunId) -> Result<Run, ReviewRefusal> {
  let stored = db.run(run).map_err(run_store_error)?.ok_or(ReviewRefusal::NotFound)?;
  if *stored.status() != RunStatus::Succeeded || db.run_merge(run)?.is_some() {
    return Err(ReviewRefusal::NotWaiting);
  }
  Ok(stored)
}

/// The thread `thread` of the project in `db`.
fn thread_in(db: &ProjectDb, thread: ReviewThreadId) -> Result<ReviewThread, ReviewRefusal> {
  db.review_thread(thread)?.ok_or(ReviewRefusal::NotFound)
}

/// Starts a comment thread on `run` of the project at `root`, about `anchor` of its diff at `head`, with the person's
/// `body` at `now`: it waits unsent in the tray. A question starts with [`ask::start`], as it goes to Implement at
/// once.
pub fn comment(
  root: &Path,
  run: RunId,
  (anchor, head): (LineAnchor, Revision),
  body: &MessageBody,
  now: Timestamp,
) -> Result<ReviewThread, ReviewRefusal> {
  let mut db = ProjectDb::open(root)?;
  waiting(&db, run)?;
  let thread = ReviewThread::start(run, (ThreadKind::Comment, anchor, head), body, now);
  db.add_review_thread(&thread)?;
  Ok(thread)
}

/// Changes the words of comment `message` while it is not sent.
pub fn edit(root: &Path, message: ReviewMessageId, body: &MessageBody) -> Result<(), ReviewRefusal> {
  if ProjectDb::open(root)?.edit_review_comment(message, body)? {
    Ok(())
  } else {
    Err(ReviewRefusal::Review(ReviewError::NotEditable))
  }
}

/// Deletes comment `message` while it is not sent; its thread goes with it when it was its only message.
pub fn delete(root: &Path, message: ReviewMessageId) -> Result<(), ReviewRefusal> {
  if ProjectDb::open(root)?.delete_review_comment(message)? {
    Ok(())
  } else {
    Err(ReviewRefusal::Review(ReviewError::NotEditable))
  }
}

/// *Reply* on comment thread `thread`, which Implement addressed: the person's `body` at `now` reopens it as a new
/// comment that waits unsent in the tray. A question is asked again with [`ask::again`].
pub fn reply(root: &Path, thread: ReviewThreadId, body: &MessageBody, now: Timestamp) -> Result<(), ReviewRefusal> {
  let mut db = ProjectDb::open(root)?;
  let stored = thread_in(&db, thread)?;
  if stored.kind() != ThreadKind::Comment {
    return Err(ReviewRefusal::Review(ReviewError::WrongKind));
  }
  waiting(&db, stored.run())?;
  let message = stored.follow_up(body, now).map_err(ReviewRefusal::Review)?;
  db.add_review_message(thread, &message)?;
  Ok(())
}

/// *Resolve* folds `thread` at `now`; `None` unfolds it again. It is kept either way.
pub fn resolve(root: &Path, thread: ReviewThreadId, at: Option<Timestamp>) -> Result<(), ReviewRefusal> {
  let mut db = ProjectDb::open(root)?;
  thread_in(&db, thread)?;
  db.resolve_review_thread(thread, at)?;
  Ok(())
}

/// Marks file `path` of `run`'s diff at `head` *Viewed* at `at`, or takes the mark off (`None`).
pub fn viewed(
  root: &Path,
  (run, head, path): (RunId, &Revision, &str),
  at: Option<Timestamp>,
) -> Result<(), ReviewRefusal> {
  ProjectDb::open(root)?.mark_review_viewed((run, head, path), at)?;
  Ok(())
}

/// Whether a comment of `threads` is on its way to Implement or being worked on, while `run` works.
fn implement_at_work(threads: &[ReviewThread], run: &Run) -> bool {
  threads
    .iter()
    .flat_map(ReviewThread::messages)
    .any(|message| match message.said() {
      Said::Comment(CommentState::Sending { .. }) => true,
      Said::Comment(CommentState::Sent { .. }) => !run.status().is_terminal(),
      _ => false,
    })
}
