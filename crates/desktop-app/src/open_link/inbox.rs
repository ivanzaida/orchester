//! Where the links the window takes arrive: the one the program was started with and those the pipe hands over, from
//! whichever thread reads them, in order, with a count the window follows.

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::watch;

use super::OpenLink;

/// The session's links. Clones share them.
#[derive(Clone)]
pub struct LinkInbox {
  arrived: Arc<Mutex<Vec<OpenLink>>>,
  count: Arc<watch::Sender<u64>>,
}

impl Default for LinkInbox {
  fn default() -> Self {
    Self {
      arrived: Arc::default(),
      count: Arc::new(watch::channel(0).0),
    }
  }
}

impl LinkInbox {
  /// A link for the window to open.
  pub fn push(&self, link: OpenLink) {
    tracing::info!(form = link.form(), "a link arrived");
    self.arrived.lock().push(link);
    self.count.send_modify(|count| *count += 1);
  }

  /// The links that arrived since the last take.
  pub fn take(&self) -> Vec<OpenLink> {
    std::mem::take(&mut *self.arrived.lock())
  }

  /// How many links arrived, as it changes.
  pub fn follow(&self) -> watch::Receiver<u64> {
    self.count.subscribe()
  }
}

/// Inboxes are the same when they share their links.
impl PartialEq for LinkInbox {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.arrived, &other.arrived)
  }
}

impl lurq::app::component::DevtoolsInspectable for LinkInbox {}
