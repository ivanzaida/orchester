//! The one send at a time of a team project's outbox. *Publish for the team*, a new resource's *Create*, the pass that
//! sends what waited and the stream's reconnect all send the versions published here: a *Cancel sending* that took a
//! waiting version back while a send of it was under way would leave it published on the server and gone here.

use std::{path::Path, sync::Arc};

use tokio::sync::{Mutex, MutexGuard};

use super::Servers;
use crate::messages::Message;

impl Servers {
  /// The lane of the project at `root`, which every holder of these `Servers` that names the same folder shares. An
  /// entry is never removed: it is one empty mutex per project folder that sent, so the map does not grow with sends.
  pub(crate) fn send_lane(&self, root: &Path) -> Arc<Mutex<()>> {
    self.inner.sending.lock().entry(root.to_owned()).or_default().clone()
  }
}

/// Waits for the lane's turn and holds it. For the blocking workers the sends run on, which end in a server's answer.
pub(crate) fn send_turn(lane: &Mutex<()>) -> MutexGuard<'_, ()> {
  lane.blocking_lock()
}

/// Takes the lane for a change of the outbox that must not meet a send; fails with what to tell the person while one is
/// under way.
pub(crate) fn between_sends(lane: &Mutex<()>) -> Result<MutexGuard<'_, ()>, Message> {
  lane.try_lock().map_err(|_| Message::new("sharing.errors.beingSent"))
}
