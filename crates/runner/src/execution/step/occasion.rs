//! Why an invocation delivers people's messages now, and how they reach its agent.

use orchester_core::{conversation::Arrival, run::MessageOccasion};

/// Why an invocation delivers people's messages now ([`MessageOccasion`], owning the question).
#[derive(Default)]
pub(in super::super) enum Occasion {
  /// Before the step started, or while it was stopped.
  #[default]
  Written,
  /// Its agent finished its turn while they came in.
  AfterTurn,
  /// A person stopped its agent's turn (*Stop turn*): they are its next instruction.
  TurnStopped,
  /// They answer the question its agent asked.
  Answer(String),
  /// A follow-up of a run that succeeded.
  FollowUp,
  /// A new message resumes a failed attempt with its earlier output as context.
  Recovery(String),
}

impl Occasion {
  /// How the messages an invocation started for this occasion reach its agent.
  pub(super) const fn arrival(&self) -> Arrival {
    match self {
      Self::AfterTurn | Self::TurnStopped => Arrival::TurnEnd,
      Self::Written | Self::Answer(_) | Self::FollowUp | Self::Recovery(_) => Arrival::Prompt,
    }
  }

  pub(super) fn as_prompt(&self) -> MessageOccasion<'_> {
    match self {
      Self::Written => MessageOccasion::Written,
      Self::AfterTurn => MessageOccasion::AfterTurn,
      Self::TurnStopped => MessageOccasion::TurnStopped,
      Self::Answer(question) => MessageOccasion::Answer { question },
      Self::FollowUp => MessageOccasion::FollowUp,
      Self::Recovery(previous_result) => MessageOccasion::Recovery { previous_result },
    }
  }
}
