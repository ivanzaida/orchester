//! How each team project's reads last ended with its server, by the project's checkout: answered, or the server did not
//! answer. The status bar's server item says the connection by it (`crate::shell::status_bar`): the reads are what a
//! team project's screens show, so they, not the project's live stream, say whether its server is reached.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use parking_lot::Mutex;
use tokio::sync::watch;

/// How a team project's reads last ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reached {
  /// The server answered the last read.
  Answered,
  /// The server could not be reached, or refused for now.
  Unanswered,
}

/// The last end of each team project's reads in the session. Clones share it.
#[derive(Clone)]
pub struct ReadsReached {
  states: Arc<Mutex<HashMap<PathBuf, Reached>>>,
  changes: Arc<watch::Sender<u64>>,
}

impl PartialEq for ReadsReached {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.states, &other.states)
  }
}

impl ReadsReached {
  pub(super) fn new() -> Self {
    Self {
      states: Arc::default(),
      changes: Arc::new(watch::channel(0).0),
    }
  }

  /// The reads of the project at `root` ended `reached`; wakes [`Self::subscribe`] when that changes what they said.
  pub(super) fn record(&self, root: &Path, reached: Reached) {
    let changed = self.states.lock().insert(root.to_owned(), reached) != Some(reached);
    if changed {
      self.changes.send_modify(|count| *count += 1);
    }
  }

  /// How the reads of the project at `root` last ended; `None` before any ended.
  pub fn of(&self, root: &Path) -> Option<Reached> {
    self.states.lock().get(root).copied()
  }

  /// Wakes whenever how a project's reads ended changes.
  pub fn subscribe(&self) -> watch::Receiver<u64> {
    self.changes.subscribe()
  }
}
