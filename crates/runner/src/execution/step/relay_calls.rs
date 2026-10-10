//! Calls held by the relay, including time spent deciding and sending their results to the CLI.
use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use tokio::sync::mpsc;

#[derive(Clone)]
pub(super) struct Calls {
  active: Arc<AtomicUsize>,
  returned: mpsc::UnboundedSender<()>,
}

impl Calls {
  pub(super) fn new() -> (Self, mpsc::UnboundedReceiver<()>) {
    let (returned, receiver) = mpsc::unbounded_channel();
    (
      Self {
        active: Arc::new(AtomicUsize::new(0)),
        returned,
      },
      receiver,
    )
  }

  pub(super) fn busy(&self) -> bool {
    self.active.load(Ordering::Acquire) > 0
  }

  pub(super) fn start(&self) -> Held {
    self.active.fetch_add(1, Ordering::AcqRel);
    Held(self.clone())
  }
}

pub(super) struct Held(Calls);

impl Drop for Held {
  fn drop(&mut self) {
    self.0.active.fetch_sub(1, Ordering::AcqRel);
    // Once the worker ended, no one needs the return notification.
    if self.0.returned.send(()).is_err() {
      tracing::debug!("the worker ended before the relay returned its call");
    }
  }
}
