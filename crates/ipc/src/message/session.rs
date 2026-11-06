//! How a runner's session relates to the desktop's: which session it serves, how the desktop's previous one ended, and
//! how the runner stops.

use orchester_core::run::InterruptCause;
use serde::{Deserialize, Serialize};

/// Which session a runner serves, from the desktop's side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AttachSession {
  /// The project's first runner in the desktop's session. Runs still recorded as running were
  /// cut off by the end of the previous session and wait for a person to continue them.
  Fresh { previous_end: SessionEnd },
  /// The desktop started this runner after the project's previous runner dropped. Runs that
  /// runner left running were interrupted by its loss and continue on their own.
  Reconnect,
}

impl AttachSession {
  /// Why a run still recorded as running when this runner attaches stopped: the previous
  /// runner's loss, Quit now, or else a crash. A run left running by a session that says it
  /// ended cleanly or with paused runs lost its runner without anyone recording it.
  pub const fn interrupt_cause(self) -> InterruptCause {
    match self {
      Self::Reconnect => InterruptCause::RunnerLost,
      Self::Fresh {
        previous_end: SessionEnd::QuitNow,
      } => InterruptCause::QuitNow,
      Self::Fresh {
        previous_end: SessionEnd::Crashed | SessionEnd::Clean | SessionEnd::RunsPaused,
      } => InterruptCause::Crashed,
    }
  }
}

/// How the desktop's previous session ended, as its session log recorded it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEnd {
  /// There was none, or it closed without active runs.
  Clean,
  /// The user waited for agents to save; the runs were paused.
  RunsPaused,
  /// The user chose Quit now.
  QuitNow,
  /// It never recorded its end: a crash, power loss or a killed process.
  Crashed,
}

/// How the runner stops, matching the close confirmation in `docs/architecture/storage.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownMode {
  /// "Wait for agents to save": stop scheduling and let active steps reach a checkpoint.
  FinishAtCheckpoints,
  /// "Quit now": terminate agents without a checkpoint.
  Immediate,
}
