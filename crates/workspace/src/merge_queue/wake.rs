//! Who follows a project's merge queue learns of its changes here: the queue's driver, so that a person's change
//! (approve, retry, a run resolved by hand) is acted on at once, and the screens that show the queue, which read it
//! again. One counter per project folder, for the whole session.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::LazyLock,
};

use parking_lot::Mutex;
use tokio::sync::watch;

static COUNTERS: LazyLock<Mutex<HashMap<PathBuf, watch::Sender<u64>>>> = LazyLock::new(Default::default);

/// Tells whoever follows the queue of the project at `root` that it changed.
pub fn wake(root: &Path) {
  counter(root).send_modify(|count| *count += 1);
}

/// The count of changes of the queue of the project at `root`, moving whenever [`wake`] is called for it.
pub fn changes(root: &Path) -> watch::Receiver<u64> {
  counter(root).subscribe()
}

fn counter(root: &Path) -> watch::Sender<u64> {
  COUNTERS
    .lock()
    .entry(root.to_path_buf())
    .or_insert_with(|| watch::Sender::new(0))
    .clone()
}
