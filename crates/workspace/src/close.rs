//! Closing the window closes the application (`docs/architecture/storage.md`, "Lifecycle").
//! Without active work the runner is stopped quietly; with active work the close is held for
//! a confirmation: wait for agents to save, quit now, or cancel. The window's close hook and the
//! confirmation's state are each app's; what a close leads to and how the session ends are here.

use std::sync::Arc;

use orchester_store::SessionEnd;

use crate::runner::{ActiveWork, ShutdownOutcome};

/// What a close request leads to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClosePlan {
  /// Nothing would be interrupted: stop the runner and close.
  StopQuietly,
  /// Ask first, listing the work in progress.
  Confirm(Arc<[ActiveWork]>),
}

pub fn close_plan(active: Vec<ActiveWork>) -> ClosePlan {
  if active.is_empty() {
    ClosePlan::StopQuietly
  } else {
    ClosePlan::Confirm(active.into())
  }
}

/// How the application was closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseKind {
  /// No work was active.
  Quiet,
  /// The user chose "Wait for agents to save".
  WaitForAgents,
  /// The user chose "Quit now".
  QuitNow,
}

/// How the session ends, given how it was closed and how the runner stopped. A runner that
/// did not confirm its stop was terminated, which is a quit-now end.
pub fn session_end(kind: CloseKind, outcome: ShutdownOutcome) -> SessionEnd {
  match (kind, outcome) {
    (CloseKind::QuitNow, _) | (_, ShutdownOutcome::Unanswered) => SessionEnd::QuitNow,
    (CloseKind::WaitForAgents, ShutdownOutcome::Stopped) => SessionEnd::RunsPaused,
    (CloseKind::Quiet, _) | (CloseKind::WaitForAgents, ShutdownOutcome::NoRunner) => SessionEnd::Clean,
  }
}
