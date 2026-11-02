use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Server-wide claim freshness duration, stored and sent as whole seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct ClaimStalenessThreshold(u64);

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("claim staleness threshold must be between 300 and 86400 seconds")]
pub struct ClaimStalenessThresholdError;

impl ClaimStalenessThreshold {
  pub const DEFAULT: Self = Self(900);

  pub const fn new(seconds: u64) -> Result<Self, ClaimStalenessThresholdError> {
    if seconds < 300 || seconds > 86400 {
      return Err(ClaimStalenessThresholdError);
    }
    Ok(Self(seconds))
  }

  pub const fn seconds(self) -> u64 {
    self.0
  }

  pub const fn millis(self) -> i64 {
    (self.0 * 1000) as i64
  }
}

impl Default for ClaimStalenessThreshold {
  fn default() -> Self {
    Self::DEFAULT
  }
}

impl TryFrom<u64> for ClaimStalenessThreshold {
  type Error = ClaimStalenessThresholdError;

  fn try_from(seconds: u64) -> Result<Self, Self::Error> {
    Self::new(seconds)
  }
}

impl From<ClaimStalenessThreshold> for u64 {
  fn from(value: ClaimStalenessThreshold) -> Self {
    value.seconds()
  }
}
