//! A thread's state as the review page reads it: the states of `RBR9O` but the two *writing* ones, which are the
//! page's own and never stored, and the *Viewed* mark of a file at a head.

use super::{CommentState, QuestionState, ReviewFailure, ReviewThread, Said};
use crate::{revision::Revision, time::Timestamp};

/// Where a thread is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreadState {
  /// A question asked; nothing from Implement yet.
  Asked,
  /// Implement is answering: its words and live activity stream in.
  Answering,
  /// Answered; no code changed.
  Answered,
  /// The person stopped the answer, with what was written.
  AnswerStopped,
  /// *Not asked*, with Orchester's reason.
  NotAsked(ReviewFailure),
  /// A comment waits in the tray.
  NotSent,
  /// *Send to Implement* is under way.
  Sending,
  /// Sent: Implement is at work on it.
  Sent,
  /// Implement's work on it ended: `commit` changed its lines (`None`: none did), with its words.
  Addressed { commit: Option<Revision> },
  /// The last send did not go, as the failure says: the comment stays unsent.
  CouldNotSend(ReviewFailure),
  /// *Resolve* folded it.
  Resolved,
  /// A later head changed its lines, in `commit`: it folds.
  Outdated { commit: Revision },
}

impl ThreadState {
  /// The state of `thread`, whose anchored lines `changed_by` changed after its head, when a later commit did. A thread
  /// Implement addressed changed its lines itself, so it is never outdated by that.
  pub fn of(thread: &ReviewThread, changed_by: Option<&Revision>) -> Self {
    if thread.resolved_at().is_some() {
      return Self::Resolved;
    }
    let addressed = thread
      .messages()
      .iter()
      .any(|message| matches!(message.said(), Said::Comment(CommentState::Addressed { .. })));
    if let Some(commit) = changed_by.filter(|_| !addressed) {
      return Self::Outdated { commit: commit.clone() };
    }
    match thread.latest_ask().said() {
      Said::Question(QuestionState::Asked) => Self::Asked,
      Said::Question(QuestionState::Answering) => Self::Answering,
      Said::Question(QuestionState::Answered) => Self::Answered,
      Said::Question(QuestionState::Stopped) => Self::AnswerStopped,
      Said::Question(QuestionState::NotAsked { failure }) => Self::NotAsked(failure.clone()),
      Said::Comment(CommentState::NotSent { failure: None }) => Self::NotSent,
      Said::Comment(CommentState::NotSent { failure: Some(failure) }) => Self::CouldNotSend(failure.clone()),
      Said::Comment(CommentState::Sending { .. }) => Self::Sending,
      Said::Comment(CommentState::Sent { .. }) => Self::Sent,
      Said::Comment(CommentState::Addressed { commit }) => Self::Addressed { commit: commit.clone() },
      Said::Agent(_) => unreachable!("the latest ask is the person's"),
    }
  }

  /// Its name, as automation reads it.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::Asked => "asked",
      Self::Answering => "answering",
      Self::Answered => "answered",
      Self::AnswerStopped => "answer_stopped",
      Self::NotAsked(_) => "not_asked",
      Self::NotSent => "not_sent",
      Self::Sending => "sending",
      Self::Sent => "sent",
      Self::Addressed { .. } => "addressed",
      Self::CouldNotSend(_) => "could_not_send",
      Self::Resolved => "resolved",
      Self::Outdated { .. } => "outdated",
    }
  }

  /// Whether the page folds it.
  pub const fn folds(&self) -> bool {
    matches!(self, Self::Resolved | Self::Outdated { .. })
  }
}

/// A file of a run's diff the person marked *Viewed* at a head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Viewed {
  pub path: String,
  pub head: Revision,
  pub at: Timestamp,
}
