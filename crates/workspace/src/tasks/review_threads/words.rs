//! What a person reads of a review: why a change was refused, and Orchester's reason a question was not asked, a
//! comment not sent or an answer ended without its words.

use orchester_core::{
  orchestrator::TurnFailure,
  review::{ReviewError, ReviewFailure},
};

use crate::messages::Message;

/// Why the thread did not take a change.
pub(super) const fn review_error(error: ReviewError) -> Message {
  Message::new(match error {
    ReviewError::NoMessages => "review.refused.noMessages",
    ReviewError::UnknownMessage => "review.refused.unknownMessage",
    ReviewError::NotEditable => "review.refused.notEditable",
    ReviewError::NotAnswered => "review.refused.notAnswered",
    ReviewError::NotAddressed => "review.refused.notAddressed",
    ReviewError::WrongKind => "review.refused.wrongKind",
  })
}

/// Orchester's reason a question was not asked or a comment not sent, as a thread shows it.
pub fn failure_message(failure: &ReviewFailure) -> Message {
  match failure {
    ReviewFailure::RunnerUnavailable => Message::new("review.failure.runnerUnavailable"),
    ReviewFailure::Lost => Message::new("review.failure.lost"),
    ReviewFailure::Refused { reason } => Message::new("review.failure.refused").with("reason", reason.clone()),
    ReviewFailure::NotWaiting => Message::new("review.failure.notWaiting"),
    ReviewFailure::NoImplementingStep => Message::new("review.failure.noImplementingStep"),
    ReviewFailure::NativeAgent => Message::new("review.failure.nativeAgent"),
    ReviewFailure::Turn(failure) => turn_message(failure),
  }
}

/// Why the answer's turn ended without its words.
fn turn_message(failure: &TurnFailure) -> Message {
  match failure {
    TurnFailure::NotSignedIn => Message::new("review.failure.notSignedIn"),
    TurnFailure::ConnectionGone => Message::new("review.failure.connectionGone"),
    TurnFailure::Account { problem } => Message::new("review.failure.account").with("detail", problem.to_string()),
    TurnFailure::CannotStart { reason } => Message::new("review.failure.cannotStart").with("detail", reason.clone()),
    TurnFailure::Failed { reason } => Message::new("review.failure.failed").with("detail", reason.clone()),
    TurnFailure::Interrupted => Message::new("review.failure.interrupted"),
    TurnFailure::TimeLimit { minutes } => Message::new("review.failure.timeLimit").with("minutes", minutes.to_string()),
  }
}
