//! What the desktop knows about the runner of the open project, as a state machine. Launching,
//! reading events and stopping happen in `connection`; every observation becomes a
//! [`RunnerEvent`] and [`RunnerState::apply`] decides what it means, including whether a
//! dropped runner is reconnected and when reconnecting gives up.

use std::{path::PathBuf, time::Duration};

/// Reconnect attempts after a drop before the runner is reported as failed: "attempt 2 of 5"
/// (status bar state E in `keJ9Z`), "Tried 5 times since 14:02" (02.H `EFY7I`).
pub const RECONNECT_LIMIT: u32 = 5;
const FIRST_RECONNECT_DELAY: Duration = Duration::from_millis(500);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(8);

/// How long to wait before reconnect attempt `attempt` (1-based): doubling from 0.5 s, at most
/// 8 s.
pub fn reconnect_delay(attempt: u32) -> Duration {
  let doublings = attempt.saturating_sub(1).min(u32::BITS - 1);
  FIRST_RECONNECT_DELAY
    .saturating_mul(1 << doublings)
    .min(MAX_RECONNECT_DELAY)
}

/// Why the runner is not available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunnerFailure {
  /// The runner program is not installed next to the desktop program.
  MissingExecutable { path: PathBuf },
  /// Another Orchester app runs this project: its runner holds the project's runner lock, or
  /// another process serves the project's runner pipe.
  AlreadyServed,
  /// The runner could not be started or did not complete the handshake; the log has the cause.
  StartFailed,
  /// The runner went away: its connection closed, a command could not be sent or it exited.
  Lost,
}

impl RunnerFailure {
  /// Retrying cannot help while the program itself is missing, or until the other app that
  /// runs the project closes; the user restarts the runner then.
  pub fn is_retryable(&self) -> bool {
    !matches!(self, Self::MissingExecutable { .. } | Self::AlreadyServed)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunnerState {
  Starting,
  Online,
  /// The runner dropped; attempt `attempt` of [`RECONNECT_LIMIT`] is waiting or running.
  Reconnecting {
    attempt: u32,
    cause: RunnerFailure,
  },
  Stopping,
  Stopped,
  /// Not running and not retrying; only a manual restart starts it again. `attempts` counts
  /// the reconnect attempts made before giving up (0 when it never got online).
  Failed {
    cause: RunnerFailure,
    attempts: u32,
  },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunnerEvent {
  Launched,
  LaunchFailed(RunnerFailure),
  /// A connected runner went away.
  Dropped(RunnerFailure),
  StopRequested,
  Stopped,
  RestartRequested,
}

impl RunnerState {
  pub fn apply(self, event: RunnerEvent) -> Self {
    use RunnerEvent as Event;
    match (self, event) {
      (Self::Starting | Self::Reconnecting { .. }, Event::Launched) => Self::Online,
      (Self::Starting, Event::LaunchFailed(cause)) | (Self::Stopping, Event::Dropped(cause)) => {
        Self::Failed { cause, attempts: 0 }
      }
      (Self::Reconnecting { attempt, .. }, Event::LaunchFailed(failure)) => Self::reconnect(attempt + 1, failure),
      (Self::Online, Event::Dropped(failure)) => Self::reconnect(1, failure),
      (Self::Starting | Self::Online | Self::Reconnecting { .. }, Event::StopRequested) => Self::Stopping,
      (Self::Stopping, Event::Stopped) => Self::Stopped,
      (Self::Failed { .. } | Self::Stopped, Event::RestartRequested) => Self::Starting,
      (state, _) => state,
    }
  }

  fn reconnect(attempt: u32, cause: RunnerFailure) -> Self {
    if cause.is_retryable() && attempt <= RECONNECT_LIMIT {
      Self::Reconnecting { attempt, cause }
    } else {
      Self::Failed {
        cause,
        attempts: attempt - 1,
      }
    }
  }

  /// The pause before the next launch, while reconnecting.
  pub fn reconnect_delay(&self) -> Option<Duration> {
    match self {
      Self::Reconnecting { attempt, .. } => Some(reconnect_delay(*attempt)),
      _ => None,
    }
  }

  /// Reconnecting after a drop, or given up: something the user should see.
  pub fn is_problem(&self) -> bool {
    matches!(self, Self::Reconnecting { .. } | Self::Failed { .. })
  }

  pub fn can_restart(&self) -> bool {
    matches!(self, Self::Failed { .. } | Self::Stopped)
  }
}

impl lurq::app::component::DevtoolsInspectable for RunnerState {}
