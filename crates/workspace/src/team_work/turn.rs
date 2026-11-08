//! The one start at a time of a team project, as a turn that is taken before a Run takes its claim: the recovery of the
//! project's open starts releases the claim of a start that failed for good, and would otherwise release the one a Run
//! has just taken for a start it has not recorded yet.

use std::path::Path;

use tokio::sync::OwnedMutexGuard;

use crate::sharing::Servers;

/// A project's turn to start or recover team starts, held until it is dropped. Every holder of the same [`Servers`]
/// that names the same folder shares one.
#[derive(Debug)]
pub struct StartTurn {
  _held: OwnedMutexGuard<()>,
}

impl StartTurn {
  /// The turn of the project at `root`, waited for.
  pub async fn wait(servers: &Servers, root: &Path) -> Self {
    Self {
      _held: servers.start_lock(root).lock_owned().await,
    }
  }

  /// The turn of the project at `root`, if no start or recovery has it.
  pub(super) fn try_take(servers: &Servers, root: &Path) -> Option<Self> {
    servers
      .start_lock(root)
      .try_lock_owned()
      .ok()
      .map(|held| Self { _held: held })
  }
}
