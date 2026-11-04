//! The rejections of requests the runner could not carry out because something it reads failed.

use orchester_ipc::Rejection;
use orchester_store::RunStoreError;

pub(super) fn failed(what: &str, error: &dyn std::error::Error) -> Rejection {
  tracing::error!("{what}: {error}");
  Rejection::Failed {
    detail: format!("{what}: {error}"),
  }
}

/// A run that could not be read: its record cannot be read back, which rejects the request for it
/// and leaves the record as it is, or reading failed.
pub(super) fn read_failed(what: &str, error: RunStoreError) -> Rejection {
  match error {
    RunStoreError::Unreadable { run, source } => {
      tracing::warn!(%run, error = %source, "a request names a run that cannot be read");
      Rejection::RunUnreadable {
        detail: source.to_string(),
      }
    }
    other => failed(what, &other),
  }
}
