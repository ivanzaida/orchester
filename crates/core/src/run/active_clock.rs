//! Active agent time as it goes on between two reads of a run: what its invocations recorded when
//! it was read, and how many of them count on since. A window shows it live from this, and the
//! next read, after the runner recorded a change, replaces it with what was recorded.

use std::time::Duration;

use super::{Invocation, Run, RunStatus};
use crate::time::Timestamp;

/// Active agent time of some invocations, read at a moment, and how it grows after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveClock {
  read_at: Timestamp,
  active: Duration,
  /// Invocations that count on: running and not waiting for a person. Agents at work at once each
  /// add their time, as the time limit counts it.
  counting: u32,
  /// Where counting on stops: the run's time limit, at which the runner stops its agents.
  limit: Option<Duration>,
}

impl ActiveClock {
  /// The active agent time of `invocations` at `now`, counting on with each that counts now.
  pub fn of<'a>(invocations: impl IntoIterator<Item = &'a Invocation>, now: Timestamp) -> Self {
    let (millis, counting) = invocations
      .into_iter()
      .fold((0_i64, 0_u32), |(millis, counting), invocation| {
        let counts = u32::from(invocation.ended_at.is_none() && invocation.timing.waiting_since().is_none());
        (millis + invocation.active_millis(now), counting + counts)
      });
    Self {
      read_at: now,
      active: Duration::from_millis(u64::try_from(millis).expect("active times are never negative")),
      counting,
      limit: None,
    }
  }

  /// What `run` lets count on: nothing once it stopped running (paused, stopped at a limit, waiting
  /// for a person, ended), and never past its time limit.
  #[must_use]
  pub fn within(self, run: &Run) -> Self {
    if *run.status() != RunStatus::Running {
      return self.stopped();
    }
    Self {
      limit: Some(run.max_duration()),
      ..self
    }
  }

  /// Counts on no more: what was read stays, e.g. while no runner looks after the run's agents.
  #[must_use]
  pub const fn stopped(self) -> Self {
    Self { counting: 0, ..self }
  }

  /// Whether the time grows after it was read.
  pub const fn counting(&self) -> bool {
    self.counting > 0
  }

  /// The active agent time at `now`: what was read, and since then the time of each invocation
  /// that counts, up to the time limit. Never less than what was read, even past the limit or for
  /// a clock set back.
  pub fn at(&self, now: Timestamp) -> Duration {
    if !self.counting() {
      return self.active;
    }
    let since = u64::try_from(now.unix_millis() - self.read_at.unix_millis()).unwrap_or_default();
    let grown = self.active + Duration::from_millis(since) * self.counting;
    match self.limit {
      Some(limit) => grown.min(limit.max(self.active)),
      None => grown,
    }
  }
}
