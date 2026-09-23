use std::time::{SystemTime, UNIX_EPOCH};

/// A UTC instant in milliseconds since the Unix epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
  pub const fn from_unix_millis(millis: i64) -> Self {
    Self(millis)
  }

  pub fn now() -> Self {
    let since_epoch = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("the system clock is set after the Unix epoch");
    Self(i64::try_from(since_epoch.as_millis()).expect("milliseconds since the epoch fit in i64 until year 292278994"))
  }

  pub const fn unix_millis(self) -> i64 {
    self.0
  }
}
