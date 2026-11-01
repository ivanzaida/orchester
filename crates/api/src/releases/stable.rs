//! The public Stable release (`GET /releases/stable`, anonymous): the release Stable offers everyone now, with its
//! files for one platform and architecture, for the website's download page. It takes no install id and counts nothing.

use serde::{Deserialize, Serialize};

use super::{FileDigest, ReleaseArch, ReleaseFileName, ReleasePlatform, ReleaseVersion};

/// What the download page asks with, sent as the query of `GET /releases/stable`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableReleaseQuery {
  pub platform: ReleasePlatform,
  pub arch: ReleaseArch,
}

impl StableReleaseQuery {
  /// The query's pairs, in the order the contract lists them.
  pub fn query(&self) -> [(&'static str, String); 2] {
    [
      ("platform", self.platform.as_str().to_owned()),
      ("arch", self.arch.as_str().to_owned()),
    ]
  }
}

/// `200` of `GET /releases/stable`: the release Stable offers everyone for the asked platform and architecture, or
/// `None` when there is none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableReleaseAnswer {
  pub release: Option<StableRelease>,
}

/// The newest release in stage `stable` at 100 %, not paused, whose files are not deleted and that has a file for the
/// asked platform and architecture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableRelease {
  pub version: ReleaseVersion,
  /// Milliseconds since the Unix epoch: when it was published.
  pub published_at: i64,
  /// Its files for the asked platform and architecture, each served at `GET /releases/{version}/files/{name}`.
  pub files: Vec<StableFile>,
}

/// One file of the public Stable release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableFile {
  pub name: ReleaseFileName,
  pub platform: ReleasePlatform,
  pub arch: ReleaseArch,
  pub size: u64,
  pub sha256: FileDigest,
}
