//! Keeps the computer awake while agents work (`docs/plan/orchestrator-autonomy/OA-08-…`): a hold that asks the
//! system not to go to sleep by itself for as long as it lives, and lets go when it is dropped. The display may turn
//! off; only the idle sleep of the system is held back. What decides when to hold is the app's, by the workspace's
//! rule; this only holds and releases.
//!
//! - Windows: `SetThreadExecutionState` with `ES_SYSTEM_REQUIRED`, which lasts while the thread that asked lives, so
//!   the hold owns a thread of its own ([`windows`]).
//! - macOS: `caffeinate -i`, the system's own tool, as a child that also ends with this process (`-w`) ([`macos`]).
//! - Any other system: not supported, and the hold says so ([`AwakeError::Unsupported`]).
//!
//! Neither keeps a laptop awake once its lid is closed on battery power: that sleep is not an idle sleep.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
use self::macos as system;
#[cfg(windows)]
use self::windows as system;

/// Why the computer is not held awake.
#[derive(Debug, thiserror::Error)]
pub enum AwakeError {
  /// The system refused, or what asks it could not be started.
  #[error("the system could not be asked to stay awake: {0}")]
  Refused(String),
  /// This system has no way Orchester knows to hold it awake.
  #[error("keeping the computer awake is not supported on this system yet")]
  Unsupported,
}

/// The computer held awake: the system does not go to sleep by itself while this lives.
#[derive(Debug)]
pub struct Awake {
  #[cfg(any(windows, target_os = "macos"))]
  _hold: system::Hold,
}

impl Awake {
  /// Asks the system to stay awake until the hold is dropped.
  pub fn hold() -> Result<Self, AwakeError> {
    #[cfg(any(windows, target_os = "macos"))]
    {
      system::Hold::take().map(|hold| Self { _hold: hold })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
      Err(AwakeError::Unsupported)
    }
  }
}
