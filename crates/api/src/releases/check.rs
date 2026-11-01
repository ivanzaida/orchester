//! The update check (`GET /updates/check`, anonymous): an app names its version, platform, architecture, channel and
//! install id, and the cloud answers the release offered to it, if any.

use orchester_core::id::InstallationId;
use serde::{Deserialize, Serialize};

use super::{
  Channel, FileDigest, FileSignature, ReleaseArch, ReleaseFileName, ReleaseKeyId, ReleasePlatform, ReleaseVersion,
};

/// What an app asks with, sent as the query of `GET /updates/check`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateCheck {
  /// The version the app runs.
  pub version: ReleaseVersion,
  pub platform: ReleasePlatform,
  pub arch: ReleaseArch,
  pub channel: Channel,
  /// The app's installation id (its `orchester.db`): what the rollout hash and the count of active installs use. It
  /// names no person or computer.
  pub install_id: InstallationId,
}

impl UpdateCheck {
  /// The query's pairs, in the order the contract lists them.
  pub fn query(&self) -> [(&'static str, String); 5] {
    [
      ("version", self.version.to_string()),
      ("platform", self.platform.as_str().to_owned()),
      ("arch", self.arch.as_str().to_owned()),
      ("channel", self.channel.as_str().to_owned()),
      ("install_id", self.install_id.to_string()),
    ]
  }
}

/// `200` of `GET /updates/check`: the release offered to this install, or `None` when nothing newer is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateCheckAnswer {
  pub update: Option<OfferedRelease>,
}

/// A release offered to one install, with its files for that install's platform and architecture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferedRelease {
  pub version: ReleaseVersion,
  /// The channel it is offered on: the install's own, or Stable for a Beta install when the newest is a Stable
  /// release.
  pub channel: Channel,
  /// Release notes in Markdown.
  pub notes: String,
  /// The app should install it without waiting: this release, or one between the install's version and it, was marked
  /// critical.
  pub critical: bool,
  /// Milliseconds since the Unix epoch: when it was published.
  pub published_at: i64,
  pub files: Vec<OfferedFile>,
}

/// One file of an offered release, with what the app checks before it runs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferedFile {
  pub name: ReleaseFileName,
  pub platform: ReleasePlatform,
  pub arch: ReleaseArch,
  /// The absolute URL of `GET /releases/{version}/files/{name}` on the cloud's API address.
  pub url: String,
  pub size: u64,
  pub sha256: FileDigest,
  pub key_id: ReleaseKeyId,
  pub signature: FileSignature,
}
