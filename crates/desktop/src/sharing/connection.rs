//! A signed-in server's connection as the status bar and Settings › Servers show it
//! (`docs/architecture/server.md`, "Live updates"; design decision 16): *connected*, *reconnecting*
//! while the backoff grows from 1 s doubling towards 60 s with jitter, *offline* once it reached
//! 60 s, and *signed out* once the server no longer accepts this computer's token, which ends the
//! retries until the person signs in.

use std::{
  hash::BuildHasher,
  time::{Duration, Instant},
};

use orchester_core::time::Timestamp;

/// How much of a wait jitter may take off, so computers that lost one server do not all retry at once.
const JITTER: f64 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connection {
  /// Signed in; the server has not answered since the application started.
  Checking,
  Connected,
  /// The `attempt`-th attempt failed; the next starts at `next`.
  Reconnecting {
    attempt: u32,
    next: Instant,
  },
  /// The backoff reached its longest wait: offline since `since`, the next attempt at `next`.
  Offline {
    since: Timestamp,
    next: Instant,
  },
  /// The server refused the token (expired or revoked), or the keychain has none.
  SignedOut,
}

impl Connection {
  /// How bad the state is, for the status bar's one item over several servers: the worst leads.
  pub const fn severity(self) -> u8 {
    match self {
      Self::Connected => 0,
      Self::Checking => 1,
      Self::Reconnecting { .. } => 2,
      Self::Offline { .. } => 3,
      Self::SignedOut => 4,
    }
  }
}

/// When the monitor asks a server again: after a failed attempt the wait starts at `first_delay`
/// and doubles up to `longest_delay`, where the server reads offline; a connected server is asked
/// every `connected_check`. The application uses [`Schedule::DEFAULT`]; tests use short ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
  pub first_delay: Duration,
  pub longest_delay: Duration,
  pub connected_check: Duration,
}

impl Schedule {
  /// 1 s doubling to 60 s, and a connected server asked every minute.
  pub const DEFAULT: Self = Self {
    first_delay: Duration::from_secs(1),
    longest_delay: Duration::from_secs(60),
    connected_check: Duration::from_secs(60),
  };

  /// How long to wait after the `attempt`-th failed attempt (1 for the first), shortened by up to a
  /// fifth by `jitter` (0 to 1).
  pub fn backoff(&self, attempt: u32, jitter: f64) -> Duration {
    self.undisturbed(attempt).mul_f64(1.0 - JITTER * jitter.clamp(0.0, 1.0))
  }

  /// The wait after the `attempt`-th failure before jitter.
  fn undisturbed(&self, attempt: u32) -> Duration {
    let doublings = attempt.saturating_sub(1).min(16);
    self.first_delay.saturating_mul(1 << doublings).min(self.longest_delay)
  }

  /// The connection after the `attempt`-th failed attempt at `now`: reconnecting until the backoff
  /// reaches its longest wait, then offline since `trouble_since`, the first failure.
  pub fn after_failure(&self, attempt: u32, now: Instant, trouble_since: Timestamp, jitter: f64) -> Connection {
    let next = now + self.backoff(attempt, jitter);
    if self.undisturbed(attempt) >= self.longest_delay {
      Connection::Offline {
        since: trouble_since,
        next,
      }
    } else {
      Connection::Reconnecting { attempt, next }
    }
  }
}

/// A random share of the jitter, from the random seed std gives every hash map.
pub fn jitter() -> f64 {
  let random = std::collections::hash_map::RandomState::new().hash_one(Instant::now());
  (random % 1_000) as f64 / 1_000.0
}
