//! What follows a turn Orchester started by itself that failed (`docs/plan/orchestrator-autonomy/OA-01-…`): the event
//! that started it stays open, and its turn is tried again after [`RETRY_AFTER`], three times at most; after the
//! fourth failure, or at once for a failure only the person can lift, the event is left to the person, where its wait
//! shows outside the chat. A turn the person stopped did not fail and is not tried again.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::chat::TurnFailure;
use crate::time::Timestamp;

/// How long after each failed turn of an event its next one may start: the first failure's, the second's, the third's.
pub const RETRY_AFTER: [Duration; 3] = [
  Duration::from_secs(60),
  Duration::from_secs(5 * 60),
  Duration::from_secs(15 * 60),
];

/// A turn of an event that failed, as its entry keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedTry {
  pub failure: TurnFailure,
  /// When the runner ended it; `None` for a turn cut short without a runner to say when.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub ended: Option<Timestamp>,
}

/// What follows an event's failed turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextTry {
  /// Its next turn may start from then on; at once for a turn nobody recorded the end of.
  From(Timestamp),
  /// No further turn starts by itself.
  LeftToPerson,
}

impl NextTry {
  /// What follows `last`, the latest failed turn of an event that had `earlier` failed turns before it.
  pub fn after(last: &FailedTry, earlier: usize) -> Self {
    if !last.failure.is_tried_again() {
      return Self::LeftToPerson;
    }
    let Some(wait) = RETRY_AFTER.get(earlier) else {
      return Self::LeftToPerson;
    };
    let wait = i64::try_from(wait.as_millis()).unwrap_or(i64::MAX);
    let ended = last.ended.map_or(0, Timestamp::unix_millis);
    Self::From(Timestamp::from_unix_millis(ended.saturating_add(wait)))
  }

  /// Whether the next turn may start at `now`.
  pub fn is_due(self, now: Timestamp) -> bool {
    matches!(self, Self::From(from) if from <= now)
  }
}

impl TurnFailure {
  /// Whether a turn Orchester started by itself is tried again after this failure: what may pass by itself is; what
  /// only the person can lift (signing in, the connection, the account) is not.
  pub const fn is_tried_again(&self) -> bool {
    match self {
      Self::CannotStart { .. } | Self::Failed { .. } | Self::Interrupted | Self::TimeLimit { .. } => true,
      Self::NotSignedIn | Self::ConnectionGone | Self::Account { .. } => false,
    }
  }
}

/// What the turn of an event is told first when an earlier turn of it failed as `last` did: that turn may have acted
/// before it ended, so this one reads the state before it acts.
pub fn retry_note(last: &FailedTry) -> String {
  let how = match &last.failure {
    TurnFailure::TimeLimit { minutes } => format!("ran longer than the {minutes} minutes an answer may take"),
    TurnFailure::Interrupted => "was cut short when Orchester or its runner closed".to_owned(),
    TurnFailure::CannotStart { .. } => "could not start".to_owned(),
    TurnFailure::Failed { .. }
    | TurnFailure::NotSignedIn
    | TurnFailure::ConnectionGone
    | TurnFailure::Account { .. } => "failed".to_owned(),
  };
  format!(
    "An earlier turn Orchester started for this same event {how}. It may have acted before it ended: read where the \
     run, its task and this conversation stand now before you act, do not repeat what is already done, and go on from \
     there."
  )
}
