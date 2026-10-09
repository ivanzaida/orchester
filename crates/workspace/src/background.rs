//! Runs blocking work (SQLite, native dialogs) on tokio's blocking pool, so the UI futures that
//! await it keep the UI thread free.

use crate::messages::{self, Message};

pub async fn run<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, Message> {
  tokio::task::spawn_blocking(work).await.map_err(|error| {
    tracing::error!(%error, "background work did not finish");
    messages::interrupted()
  })
}
