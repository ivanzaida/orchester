//! The one start at a time of a team project: a Run and the recovery of the project's open starts (`team_work`) would
//! otherwise carry on the same start together, and the one that loses the race fails on the other's records.

use std::{path::Path, sync::Arc};

use orchester_core::id::OperationId;

use super::Servers;

/// What recovery found wrong with a start, which it says once each per session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum StartTrouble {
  /// The start's record cannot be read.
  Unreadable,
  /// The start could not go on for now: the database was busy, or the server answered strangely.
  Retrying,
}

impl Servers {
  /// The lock of the project at `root`, held across the awaits of a team start. Every holder of these `Servers` that
  /// names the same folder gets the same lock.
  pub(crate) fn start_lock(&self, root: &Path) -> Arc<tokio::sync::Mutex<()>> {
    self.inner.starting.lock().entry(root.to_owned()).or_default().clone()
  }

  /// Whether this is the first time this session that start `operation` was found in `trouble`.
  pub(crate) fn first_trouble(&self, operation: OperationId, trouble: StartTrouble) -> bool {
    self.inner.told_starts.lock().insert((operation, trouble))
  }
}
