//! Which installs a Stable release reaches while it rolls out (`docs/cloud-api.md`, "Rollout"): each install falls in
//! one of 100 buckets per release, and a release at `percent` reaches the buckets below it. Raising the percentage
//! only adds installs; each release spreads its buckets anew, so the same installs are not always first.

use orchester_core::id::InstallationId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ReleaseFormatError, ReleaseVersion};

/// Keeps the rollout hash apart from every other hash of an install id.
const ROLLOUT_LABEL: &[u8] = b"orchester-rollout-v1\0";

/// The share of installs a Stable release is offered to, 0–100.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct RolloutPercent(u8);

impl RolloutPercent {
  pub const ALL: Self = Self(100);

  pub fn new(percent: u8) -> Result<Self, ReleaseFormatError> {
    if percent > 100 {
      return Err(ReleaseFormatError::Percent);
    }
    Ok(Self(percent))
  }

  pub const fn get(self) -> u8 {
    self.0
  }

  /// Whether the release `version` at this percentage reaches `install`.
  pub fn includes(self, install: InstallationId, version: &ReleaseVersion) -> bool {
    rollout_bucket(install, version) < self.0
  }
}

impl TryFrom<u8> for RolloutPercent {
  type Error = ReleaseFormatError;

  fn try_from(percent: u8) -> Result<Self, Self::Error> {
    Self::new(percent)
  }
}

impl From<RolloutPercent> for u8 {
  fn from(percent: RolloutPercent) -> Self {
    percent.0
  }
}

/// The bucket, 0–99, of `install` for the release `version`: the first 8 bytes, big-endian, of
/// SHA-256(`"orchester-rollout-v1\0"` ‖ the install id as 36 lowercase hyphenated characters ‖ `"\0"` ‖ the version
/// as written), modulo 100.
pub fn rollout_bucket(install: InstallationId, version: &ReleaseVersion) -> u8 {
  let mut hash = Sha256::new();
  hash.update(ROLLOUT_LABEL);
  hash.update(install.to_string().as_bytes());
  hash.update(b"\0");
  hash.update(version.as_str().as_bytes());
  let hash: [u8; 32] = hash.finalize().into();
  let mut first = [0_u8; 8];
  first.copy_from_slice(&hash[..8]);
  u8::try_from(u64::from_be_bytes(first) % 100).expect("a remainder of 100 fits in a byte")
}
