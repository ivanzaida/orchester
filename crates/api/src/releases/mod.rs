//! App releases between the desktop app, CI and Orchester Cloud (`docs/cloud-api.md`, "App releases"): the update
//! check, the public Stable release, downloads, CI's upload, the staff's admin, and the signed message that ties a file
//! to its release. With the `release-signature` feature, the check of a file's signature against the accepted release
//! keys; with `release-signing`, CI's signing of a file with a release key.

mod admin;
mod check;
mod ci_run;
mod hex;
mod progress;
mod rollout;
#[cfg(feature = "release-signing")]
mod sign;
mod signed;
mod stable;
mod upload;
#[cfg(feature = "release-signature")]
mod verify;
mod version;

pub use admin::{
  AdminFile, AdoptionDay, ChannelInstalls, PercentChange, ReleaseAction, ReleaseActivity, ReleaseChanges,
  ReleaseDetails, ReleaseDetailsResponse, ReleaseResponse, ReleaseStage, ReleaseSummary, ReleasesResponse, Withdrawal,
};
pub use check::{OfferedFile, OfferedRelease, UpdateCheck, UpdateCheckAnswer};
pub use ci_run::{CiRun, CiRunUrl};
pub use progress::{FileProgress, UploadState};
pub use rollout::{RolloutPercent, rollout_bucket};
#[cfg(feature = "release-signing")]
pub use sign::{ReleaseSigningKey, SigningKeyError};
pub use signed::{FileDigest, FileSignature, ReleaseKeyId, SIGNED_MESSAGE_HEADER, SignedFile};
pub use stable::{StableFile, StableRelease, StableReleaseAnswer, StableReleaseQuery};
pub use upload::{DeclaredFile, FileResult, FileUpload, FileVerdict, NewRelease, Rejection, Verification};
#[cfg(feature = "release-signature")]
pub use verify::{AcceptedReleaseKeys, ReleasePublicKey, SignatureError};
pub use version::{Channel, ReleaseArch, ReleaseFileName, ReleasePlatform, ReleaseVersion};

/// A value that is not in the release contract's format.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReleaseFormatError {
  #[error("not a release version: MAJOR.MINOR.PATCH with an optional pre-release suffix")]
  Version,
  #[error("not a release file name")]
  FileName,
  #[error("a SHA-256 is 64 lowercase hex characters")]
  Digest,
  #[error("a release key id is 16 lowercase hex characters")]
  KeyId,
  #[error("a release public key is 64 lowercase hex characters")]
  PublicKey,
  #[error("a signature is 128 lowercase hex characters")]
  Signature,
  #[error("a rollout percentage is 0 to 100")]
  Percent,
  #[error("a CI run link is an https URL of at most 2048 printable characters without spaces")]
  CiRunUrl,
}
