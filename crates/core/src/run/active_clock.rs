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
  /// Time of the invocations that ended or wait for a person: the runner recorded all of it.
  fixed: Duration,
  /// Time of the invocations that count on, up to `read_at`.
  running: Duration,
  /// Invocations that count on: running and not waiting for a person. Agents at work at once each
  /// add their time, as the time limit counts it.
  counting: u32,
  /// Where counting on stops: the run's time limit, at which the runner stops its agents.
  limit: Option<Duration>,
}

impl ActiveClock {
  /// The active agent time of `invocations` at `now`, counting on with each that counts now.
  pub fn of<'a>(invocations: impl IntoIterator<Item = &'a Invocation>, now: Timestamp) -> Self {
    let mut clock = Self {
      read_at: now,
      fixed: Duration::ZERO,
      running: Duration::ZERO,
      counting: 0,
      limit: None,
    };
    for invocation in invocations {
      let active =
        Duration::from_millis(u64::try_from(invocation.active_millis(now)).expect("active times are never negative"));
      if invocation.ended_at.is_none() && invocation.timing.waiting_since().is_none() {
        clock.running += active;
        clock.counting += 1;
      } else {
        clock.fixed += active;
      }
    }
    clock
  }

  /// What `run` lets count on: nothing once it stopped running (paused, stopped at a limit, waiting
  /// for a person, ended), and never past its time limit.
  #[must_use]
  pub fn within(self, run: &Run) -> Self {
    if *run.status() != RunStatus::Running {
      return self.stopped();
    }
    Self {
      limit: run.max_duration(),
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
  /// that counts, never past the time limit unless the ended invocations already went past it. A
  /// clock set back reads as the moment of the read.
  pub fn at(&self, now: Timestamp) -> Duration {
    if !self.counting() {
      return self.fixed + self.running;
    }
    let since = u64::try_from(now.unix_millis() - self.read_at.unix_millis()).unwrap_or_default();
    let total = self.fixed + self.running + Duration::from_millis(since) * self.counting;
    match self.limit {
      Some(limit) => total.min(limit.max(self.fixed)),
      None => total,
    }
  }
}
