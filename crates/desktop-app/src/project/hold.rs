//! Holds the project feeds' reads, so a screen's loading state stays on screen long enough for QA to
//! capture it (`AGENTS.md` rule 15). Debug builds turn it on and off through the app's MCP tool
//! `hold_reads`; nothing else holds reads, so in a release build every read goes straight through.

use std::sync::Arc;

use tokio::sync::watch;

/// Whether reads wait. Clones share it.
#[derive(Clone, Default)]
pub struct ReadHold(Arc<watch::Sender<bool>>);

impl PartialEq for ReadHold {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl ReadHold {
  /// Holds the reads that start from now on (`true`), or lets every waiting read go (`false`).
  pub fn set(&self, held: bool) {
    self.0.send_replace(held);
  }

  pub fn is_held(&self) -> bool {
    *self.0.borrow()
  }

  /// Returns once reads are not held.
  pub async fn released(&self) {
    let mut held = self.0.subscribe();
    if let Err(error) = held.wait_for(|held| !*held).await {
      // The sender is `self`'s, so the channel cannot close while this waits; the read goes ahead.
      tracing::error!(%error, "the read hold closed while a read waited");
    }
  }
}
