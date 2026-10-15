use std::time::{SystemTime, UNIX_EPOCH};

/// The cloud's time is injectable so expiry needs no wall-clock waits in tests. The database's `now()` is never
/// used for the same reason.
pub trait Clock: Send + Sync {
  fn now_ms(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
  fn now_ms(&self) -> i64 {
    let elapsed = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("the cloud's clock is after 1970");
    i64::try_from(elapsed.as_millis()).expect("Unix milliseconds fit i64 for millions of years")
  }
}

pub(crate) const MINUTE_MS: i64 = 60 * 1000;
pub(crate) const HOUR_MS: i64 = 60 * MINUTE_MS;
pub(crate) const DAY_MS: i64 = 24 * HOUR_MS;

pub(crate) fn seconds(ms: i64) -> i64 {
  ms.div_euclid(1000)
}
