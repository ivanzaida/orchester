//! How each shared project on this computer stands with its server (`docs/architecture/server.md`, "Live updates",
//! "The sync engine"): *live* while its stream is open, *reconnecting* while the stream waits to connect again,
//! *offline* once that wait reached its longest, the connection monitor's [`Schedule`], and *not a member* once the
//! server said so. A project whose server signed this computer out has no state here: its server's connection says it.
//! The status bar and the MCP tools read it; a change wakes [`Servers::subscribe`].

use std::{
  collections::{HashMap, HashSet},
  path::{Path, PathBuf},
  time::Instant,
};

use orchester_core::time::Timestamp;

use crate::sharing::{Connection, Schedule, Servers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncState {
  /// The stream is open: the cache takes every change as it is made.
  Live,
  /// The stream is closed and connects again; `attempt` connects failed or dropped in a row.
  Reconnecting { attempt: u32 },
  /// The stream could not connect since `since`, and tries again every minute.
  Offline { since: Timestamp },
  /// The server no longer has the person as a member; nothing follows it until *Check again* finds them one.
  NotMember,
}

impl SyncState {
  /// The state after the `attempt`-th connect in a row failed or dropped, the first at `since`.
  pub(super) fn after_failure(attempt: u32, since: Timestamp) -> Self {
    match Schedule::DEFAULT.after_failure(attempt, Instant::now(), since, 0.0) {
      Connection::Offline { since, .. } => Self::Offline { since },
      _ => Self::Reconnecting { attempt },
    }
  }
}

/// The state of each project by its folder.
pub(in crate::sharing) type SyncStates = HashMap<PathBuf, SyncState>;

impl Servers {
  /// How the shared project at `root` stands with its server; `None` while it has no stream, e.g. signed out.
  pub fn sync_state(&self, root: &Path) -> Option<SyncState> {
    self.inner.sync_states.lock().get(root).copied()
  }

  /// [`Self::sync_state`] of every shared project that has one, by folder.
  pub fn sync_states(&self) -> HashMap<PathBuf, SyncState> {
    self.inner.sync_states.lock().clone()
  }

  /// The stream of the project at `root` reached `state`.
  pub(super) fn set_sync_state(&self, root: &Path, state: SyncState) {
    let previous = self.inner.sync_states.lock().insert(root.to_owned(), state);
    if previous != Some(state) {
      self.changed();
    }
  }

  /// Keeps the states of the projects at `followed`, whose streams run, and says *not a member* for those at
  /// `not_members`; every other project's state goes.
  pub(super) fn keep_sync_states(&self, followed: &HashSet<PathBuf>, not_members: Vec<PathBuf>) {
    let changed = {
      let mut states = self.inner.sync_states.lock();
      let before = states.clone();
      states.retain(|root, _| followed.contains(root));
      states.extend(not_members.into_iter().map(|root| (root, SyncState::NotMember)));
      *states != before
    };
    if changed {
      self.changed();
    }
  }
}
