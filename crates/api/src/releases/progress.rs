//! A draft's files as the admin sees them while CI uploads them (`docs/cloud-api.md`, "Uploading a release"): the parts
//! that arrived, the check at finalizing, and why a file was rejected.

use serde::{Deserialize, Serialize};

use super::{FileDigest, Rejection, ReleaseArch, ReleaseFileName, ReleasePlatform};

/// One declared file of a draft and how far its upload is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileProgress {
  pub name: ReleaseFileName,
  pub platform: ReleasePlatform,
  pub arch: ReleaseArch,
  pub size: u64,
  /// The SHA-256 CI declared.
  pub sha256: FileDigest,
  /// How many parts the file has.
  pub parts: u32,
  /// How many of them the cloud has.
  pub received: u32,
  #[serde(flatten)]
  pub state: UploadState,
}

/// Where a file of a draft is between its declaration and the release's publishing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UploadState {
  /// Parts are still arriving, or all arrived and CI has not finalized yet.
  Receiving,
  /// Finalizing is checking its SHA-256 and signature.
  Verifying,
  /// Finalizing accepted it; the release is published once every file is.
  Verified,
  /// Finalizing rejected it; its parts were dropped and CI sends it again.
  Rejected {
    reason: Rejection,
    /// The SHA-256 of the bytes that arrived; `None` when not every part did (`incomplete`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    received_sha256: Option<FileDigest>,
  },
}
