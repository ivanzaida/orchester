pub(crate) mod access;
pub(crate) mod batches;
pub(crate) mod claims;
pub(crate) mod events;
pub(crate) mod limits;
pub(crate) mod managed;
pub(crate) mod members;
pub(crate) mod organization;
pub(crate) mod projects;
pub(crate) mod tags;
pub(crate) mod task_rows;
pub(crate) mod task_write;

use orchester_api::{ApiError, ErrorCode, SharingError, SharingErrorDetails};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
  Server,
  error::{ServerError, status_message},
};

impl Server {
  /// Read time after acquiring the snapshot; queued work must not use an earlier clock.
  pub(crate) async fn snapshot<T: Send + 'static>(
    &self,
    operation: impl FnOnce(&rusqlite::Connection, i64) -> Result<T, ServerError> + Send + 'static,
  ) -> Result<T, ServerError> {
    let clock = self.inner.clock.clone();
    self.inner.db.snapshot(move |db| operation(db, clock.now_ms())).await
  }

  /// Claim freshness and session expiry are decided after the write transaction starts.
  pub(crate) async fn write<T: Send + 'static>(
    &self,
    operation: impl FnOnce(&rusqlite::Connection, i64) -> Result<T, ServerError> + Send + 'static,
  ) -> Result<T, ServerError> {
    let clock = self.inner.clock.clone();
    let state = self.inner.clone();
    self
      .inner
      .db
      .write(move |db| {
        let now = clock.now_ms();
        let result = operation(db, now)?;
        if now >= state.event_prune_at.load(std::sync::atomic::Ordering::Relaxed) {
          events::prune(db, now)?;
          // Writes are serialized by the database; replay excludes expired rows even if a commit fails.
          state
            .event_prune_at
            .store(now.saturating_add(60_000), std::sync::atomic::Ordering::Relaxed);
        }
        Ok(result)
      })
      .await
  }
}

pub(crate) fn conflict(code: ErrorCode, details: SharingErrorDetails) -> ServerError {
  ServerError::Sharing(Box::new(SharingError {
    error: ApiError {
      error: code,
      message: status_message(code).1.into(),
    },
    details,
  }))
}

pub(crate) fn reason(code: ErrorCode, reason: &str) -> ServerError {
  conflict(code, SharingErrorDetails::Reason { reason: reason.into() })
}

pub(crate) fn encode(value: &impl Serialize) -> Result<String, ServerError> {
  serde_json::to_string(value).map_err(|_| ServerError::StoredData)
}
pub(crate) fn decode<T: DeserializeOwned>(value: &str) -> Result<T, ServerError> {
  serde_json::from_str(value).map_err(|_| ServerError::StoredData)
}
pub(crate) fn sql_integer(value: u64) -> Result<i64, ServerError> {
  i64::try_from(value).map_err(|_| ErrorCode::Invalid.into())
}
pub(crate) fn unsigned(value: i64) -> Result<u64, ServerError> {
  u64::try_from(value).map_err(|_| ServerError::StoredData)
}
