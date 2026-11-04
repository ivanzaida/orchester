//! How the runners' requests and their stopping ended.

use std::time::Duration;

use orchester_core::id::RunId;
use orchester_ipc::{RefusedRun, Rejection};

use super::RequestError;

/// How long a close without active work waits for the runner to stop and exit before
/// terminating it.
pub const QUIET_STOP_TIMEOUT: Duration = Duration::from_secs(6);

/// How asking the runners to stop ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownOutcome {
  /// No runner was running.
  NoRunner,
  /// Every runner confirmed that it stopped.
  Stopped,
  /// A runner did not confirm in time or went away first; it was terminated.
  Unanswered,
}

/// Why the runner did not do what a request about a run asked.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RunRequestError {
  #[error(transparent)]
  Request(#[from] RequestError),
  #[error("the runner refused the request: {0:?}")]
  Rejected(Rejection),
}

/// The answer to continuing interrupted runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Continued {
  pub continued: Vec<RunId>,
  pub refused: Vec<RefusedRun>,
}
