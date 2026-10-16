use std::time::{SystemTime, UNIX_EPOCH};

/// Server time is injectable so expiry does not need wall-clock waits in tests.
pub trait Clock: Send + Sync {
  fn now_ms(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
  fn now_ms(&self) -> i64 {
    let elapsed = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("server clock is after 1970");
    i64::try_from(elapsed.as_millis()).expect("Unix milliseconds fit i64 for millions of years")
  }
}

/// A device token expires after 90 days without use (row 56).
pub const DEVICE_IDLE_MS: i64 = 90 * 24 * 60 * 60 * 1000;
/// A browser's sign-in to the server admin ends 12 hours after it began, used or not.
pub const ADMIN_WEB_MS: i64 = 12 * 60 * 60 * 1000;
