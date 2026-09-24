//! How the runner state and the activity log become the status bar's runner item and
//! activity slot (states A–H in `keJ9Z`).

use super::log::{ActivityLog, Slot};
use crate::runner::state::RunnerState;

/// The runner item (`N8rki`): which icon and label it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerBadge {
  /// B: loader, "Starting runner…".
  Starting,
  /// A, C, D, F, H: plug, "Runner online".
  Online,
  /// E: refresh, "Reconnecting to runner" with the attempt and the next one.
  Reconnecting,
  /// Stopping for a project switch or the close.
  Stopping,
  Stopped,
  /// G: unplug, "Runner offline"; only a restart starts it again.
  Offline,
}

impl RunnerBadge {
  pub fn of(state: &RunnerState) -> Self {
    match state {
      RunnerState::Starting => Self::Starting,
      RunnerState::Online => Self::Online,
      RunnerState::Reconnecting { .. } => Self::Reconnecting,
      RunnerState::Stopping => Self::Stopping,
      RunnerState::Stopped => Self::Stopped,
      RunnerState::Failed { .. } => Self::Offline,
    }
  }
}

/// What the activity slot (`IRKYz` and its actions) shows.
#[derive(Debug, PartialEq, Eq)]
pub enum ActivitySlot<'a> {
  /// B: nothing while the runner starts.
  Hidden,
  /// E, G: the runner problem, with G's actions.
  RunnerProblem(RunnerBadge),
  /// A, C, D, F, H: the log's own priority.
  Activity(Slot<'a>),
}

/// `runner` is `None` when no project is open (first launch, setup): there is no runner to
/// report, only the activity.
pub fn activity_slot<'a>(runner: Option<&RunnerState>, log: &'a ActivityLog) -> ActivitySlot<'a> {
  match runner {
    Some(RunnerState::Starting) => ActivitySlot::Hidden,
    // A runner problem outranks everything else in the slot.
    Some(state) if state.is_problem() => ActivitySlot::RunnerProblem(RunnerBadge::of(state)),
    _ => ActivitySlot::Activity(log.slot()),
  }
}
