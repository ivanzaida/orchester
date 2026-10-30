//! Releases as Orchester staff see and steer them in the cloud's admin (`docs/cloud-api.md`, "Release admin", a staff
//! account's two-factor session): where each release is, how many installs run it, and the moves between stages.

use serde::{Deserialize, Serialize};

use super::{
  Channel, FileDigest, ReleaseArch, ReleaseFileName, ReleaseKeyId, ReleasePlatform, ReleaseVersion, RolloutPercent,
};

/// Where a release is. A draft is being uploaded; a finalized release is on Beta until staff promote it to Stable at a
/// percentage; a withdrawn one is offered to nobody and its files are no longer served.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum ReleaseStage {
  Draft,
  Beta,
  /// On Stable for `percent` of installs ([`super::rollout_bucket`]), and on Beta for all; a paused one is offered to
  /// no Stable install until it is resumed.
  Stable {
    percent: RolloutPercent,
    paused: bool,
  },
  Withdrawn,
}

impl ReleaseStage {
  /// The channels the release is offered on now (a paused Stable release still on Beta).
  pub fn channels(self) -> &'static [Channel] {
    match self {
      Self::Draft | Self::Withdrawn => &[],
      Self::Beta => &[Channel::Beta],
      Self::Stable { .. } => &[Channel::Stable, Channel::Beta],
    }
  }
}

/// A release in the admin's list, and the answer of each change to one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseSummary {
  pub version: ReleaseVersion,
  #[serde(flatten)]
  pub stage: ReleaseStage,
  pub critical: bool,
  /// Milliseconds since the Unix epoch.
  pub created_at: i64,
  /// When it was finalized; `None` for a draft.
  pub published_at: Option<i64>,
  /// Active installs that run it: distinct install ids whose newest update check in the last 30 days named this
  /// version.
  pub installs: u64,
  /// Staff deleted its files; the release stays in the list and its history.
  pub files_deleted: bool,
}

/// `GET /admin/releases`: every release, newest version first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasesResponse {
  pub releases: Vec<ReleaseSummary>,
}

/// The answer of each change to a release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseResponse {
  pub release: ReleaseSummary,
}

/// `GET /admin/releases/{version}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDetailsResponse {
  pub release: ReleaseDetails,
}

/// One release with its files, adoption and history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDetails {
  #[serde(flatten)]
  pub summary: ReleaseSummary,
  /// Release notes in Markdown.
  pub notes: String,
  pub files: Vec<AdminFile>,
  /// Installs that ran it on each of the last 30 days (UTC), oldest first.
  pub adoption: Vec<AdoptionDay>,
  /// Everything done to it, newest first.
  pub activity: Vec<ReleaseActivity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminFile {
  pub name: ReleaseFileName,
  pub platform: ReleasePlatform,
  pub arch: ReleaseArch,
  pub size: u64,
  pub sha256: FileDigest,
  pub key_id: ReleaseKeyId,
  /// Downloads that started at byte 0; resumed ranges do not count again.
  pub downloads: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptionDay {
  /// `YYYY-MM-DD`, UTC.
  pub day: String,
  /// Distinct install ids whose update checks that day named this version.
  pub installs: u64,
}

/// One entry of a release's history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseActivity {
  /// Milliseconds since the Unix epoch.
  pub at: i64,
  /// The staff member's display name; `None` for CI's release token.
  pub actor: Option<String>,
  #[serde(flatten)]
  pub action: ReleaseAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ReleaseAction {
  Created,
  /// Finalizing rejected at least one file; the release stayed a draft.
  Rejected,
  Published,
  Promoted {
    percent: RolloutPercent,
  },
  PercentSet {
    percent: RolloutPercent,
  },
  Paused,
  Resumed,
  Withdrawn,
  CriticalSet {
    critical: bool,
  },
  NotesEdited,
  FilesDeleted,
}

/// `POST /admin/releases/{version}/promote` and `PUT /admin/releases/{version}/rollout`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PercentChange {
  pub percent: RolloutPercent,
}

/// `PATCH /admin/releases/{version}`: what changes; an absent field stays.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseChanges {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub notes: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub critical: Option<bool>,
}
