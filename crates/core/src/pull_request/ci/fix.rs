//! Fix CI (`docs/design/pr-ci-status.md`): the failing checks of a run's pull request sent to its agent as one
//! message, once by hand or by *Fix CI automatically*, a switch of the run with a limit of fixes. A Fix CI answers
//! one failure, told by the failing checks' names and pages ([`failure_of`]): a new push runs new checks with new
//! pages, so its failure is a new one. The automatic sends count from the switch's last change, so changing the
//! limit restarts the count; a send by hand never counts.

use super::{CiChecks, CiState};
use crate::{id::MessageId, time::Timestamp};

/// How many fixes *Fix CI automatically* sends for a run before it stops and leaves the run to the person.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FixLimit {
  One,
  Two,
  #[default]
  Three,
  Five,
}

impl FixLimit {
  /// Every limit the switch's select offers, in its order.
  pub const ALL: [Self; 4] = [Self::One, Self::Two, Self::Three, Self::Five];

  pub const fn get(self) -> u32 {
    match self {
      Self::One => 1,
      Self::Two => 2,
      Self::Three => 3,
      Self::Five => 5,
    }
  }

  /// The limit of `count` fixes, if the switch offers it.
  pub fn of(count: u32) -> Option<Self> {
    Self::ALL.into_iter().find(|limit| limit.get() == count)
  }
}

/// *Fix CI automatically* of a run: off by default, with its limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutoFix {
  pub on: bool,
  pub limit: FixLimit,
}

/// Who sent a Fix CI: the person, or the switch, as fix `count` of its `limit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedBy {
  Hand,
  Automatically { count: u32, limit: FixLimit },
}

/// What a recorded Fix CI event did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixEventKind {
  /// The failing checks went to the run's agent as message `message`.
  Sent { message: MessageId, by: FixedBy },
  /// *Fix CI automatically* reached its limit on a failure it did not send: the run waits for the person.
  Stopped { limit: FixLimit },
}

/// One Fix CI of a run, for the Conversation's event lines and the switch's count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixEvent {
  pub at: Timestamp,
  /// The pull request whose checks failed.
  pub number: u64,
  /// The failure it answers ([`failure_of`]).
  pub failure: String,
  /// The failing checks' names, in `gh`'s order.
  pub failing: Vec<String>,
  pub kind: FixEventKind,
}

/// A run's Fix CI: its switch, when the switch last changed, and the events, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunFixes {
  pub auto: AutoFix,
  /// When the switch or its limit last changed; `None` while it was never set.
  pub changed_at: Option<Timestamp>,
  pub events: Vec<FixEvent>,
}

/// What *Fix CI automatically* does about a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoStep {
  /// It sends fix `count` of its limit.
  Send { count: u32, limit: FixLimit },
  /// It reached its limit: it records that it stopped and sends nothing more.
  Stop { limit: FixLimit },
  /// It is off, or the failure was answered or stopped at already.
  Nothing,
}

impl RunFixes {
  /// The automatic fixes sent since the switch last changed.
  pub fn used(&self) -> u32 {
    let since = self.changed_at.map_or(i64::MIN, Timestamp::unix_millis);
    let sent = self.events.iter().filter(|event| {
      event.at.unix_millis() >= since
        && matches!(
          event.kind,
          FixEventKind::Sent {
            by: FixedBy::Automatically { .. },
            ..
          }
        )
    });
    u32::try_from(sent.count()).unwrap_or(u32::MAX)
  }

  /// The last Fix CI sent for `failure`, by hand or automatically.
  pub fn sent_for(&self, failure: &str) -> Option<&FixEvent> {
    self
      .events
      .iter()
      .rev()
      .find(|event| event.failure == failure && matches!(event.kind, FixEventKind::Sent { .. }))
  }

  /// The last Fix CI sent, of any failure.
  pub fn last_sent(&self) -> Option<&FixEvent> {
    self
      .events
      .iter()
      .rev()
      .find(|event| matches!(event.kind, FixEventKind::Sent { .. }))
  }

  /// What the switch does about `failure` now.
  pub fn auto_step(&self, failure: &str) -> AutoStep {
    let handled = self.events.iter().any(|event| event.failure == failure);
    if !self.auto.on || handled {
      return AutoStep::Nothing;
    }
    let limit = self.auto.limit;
    let used = self.used();
    if used >= limit.get() {
      AutoStep::Stop { limit }
    } else {
      AutoStep::Send { count: used + 1, limit }
    }
  }

  /// Whether the switch stopped at its limit since it last changed.
  pub fn limit_reached(&self) -> bool {
    let since = self.changed_at.map_or(i64::MIN, Timestamp::unix_millis);
    self.auto.on
      && self
        .events
        .iter()
        .any(|event| event.at.unix_millis() >= since && matches!(event.kind, FixEventKind::Stopped { .. }))
  }
}

/// Whether `state` is a failing check's: failed, or cancelled, which fails the pull request as well.
pub const fn is_failing(state: CiState) -> bool {
  matches!(state, CiState::Failed | CiState::Cancelled)
}

/// The failure `checks` show, told by each failing check's name and page; `None` when none fails.
pub fn failure_of(checks: &CiChecks) -> Option<String> {
  let mut failing: Vec<String> = checks
    .checks
    .iter()
    .filter(|check| is_failing(check.state))
    .map(|check| format!("{}\u{1f}{}", check.name, check.url.as_deref().unwrap_or_default()))
    .collect();
  if failing.is_empty() {
    return None;
  }
  failing.sort();
  Some(failing.join("\u{1e}"))
}
