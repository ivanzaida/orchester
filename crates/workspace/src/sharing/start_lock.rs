//! The one start at a time of a team project: a Run and the recovery of the project's open starts (`team_work`) would
//! otherwise carry on the same start together, and the one that loses the race fails on the other's records.

use std::{path::Path, sync::Arc};

use super::Servers;

impl Servers {
  /// The lock of the project at `root`, held across the awaits of a team start. Every holder of these `Servers` that
  /// names the same folder gets the same lock.
  pub(crate) fn start_lock(&self, root: &Path) -> Arc<tokio::sync::Mutex<()>> {
    self.inner.starting.lock().entry(root.to_owned()).or_default().clone()
  }
}
