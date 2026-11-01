//! Releases as Orchester staff see and steer them in the cloud's admin (`docs/cloud-api.md`, "Release admin", a staff
//! account's two-factor session): where each release is, how many installs run it, and the moves between stages.

use serde::{Deserialize, Serialize};

use super::{
  Channel, CiRun, FileDigest, FileProgress, ReleaseArch, ReleaseFileName, ReleaseKeyId, ReleasePlatform,
  ReleaseVersion, RolloutPercent,
};

/// Where a release is. A draft is being uploaded; a finalized release is on Beta until staff promote it to Stable at a
/// percentage; a withdrawn one is offered to nobody and its files are no longer served.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum ReleaseStage {
  Draft,
  /// On Beta for all installs; a paused one is offered to no Beta install until it is resumed. A cloud before Beta
  /// pausing leaves `paused` out.
  Beta {
    #[serde(default)]
    paused: bool,
  },
  /// On Stable for `percent` of installs ([`super::rollout_bucket`]), and on Beta for all; a paused one is offered to
  /// no Stable install until it is resumed.
  Stable {
    percent: RolloutPercent,
    paused: bool,
  },
  Withdrawn,
}

impl ReleaseStage {
  /// The channels the release is on (a paused release still on its channels).
  pub fn channels(self) -> &'static [Channel] {
    match self {
      Self::Draft | Self::Withdrawn => &[],
      Self::Beta { .. } => &[Channel::Beta],
      Self::Stable { .. } => &[Channel::Stable, Channel::Beta],
    }
  }
}

/// Active installs split by the channel each follows, as its newest update check named it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelInstalls {
  pub stable: u64,
  pub beta: u64,
}

impl ChannelInstalls {
  pub const fn total(self) -> u64 {
    self.stable + self.beta
  }
}

/// When a release was withdrawn, and by whom.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Withdrawal {
  /// Milliseconds since the Unix epoch.
  pub at: i64,
  /// The staff member's display name.
  pub by: String,
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
  /// `installs` by channel; `None` from a cloud before it counted them apart.
  #[serde(default)]
  pub installs_by_channel: Option<ChannelInstalls>,
  /// Staff deleted its files; the release stays in the list and its history.
  pub files_deleted: bool,
  /// The CI run that uploaded it; `None` when CI named none.
  #[serde(default)]
  pub ci_run: Option<CiRun>,
  /// Set with stage `withdrawn`; `None` otherwise, and from a cloud before it kept it.
  #[serde(default)]
  pub withdrawn: Option<Withdrawal>,
  /// A draft's declared files with how far each upload is, and the files the last finalizing rejected until CI
  /// declares them again; empty once the release is published.
  #[serde(default)]
  pub uploads: Vec<FileProgress>,
}

/// `GET /admin/releases`: every release, newest version first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasesResponse {
  pub releases: Vec<ReleaseSummary>,
  /// Every active install (an install id with an update check in the last 30 days) by channel, whatever it runs;
  /// `None` from a cloud before it counted them apart.
  #[serde(default)]
  pub installs_by_channel: Option<ChannelInstalls>,
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
  /// `installs` by the channel each check named; `None` from a cloud before it counted them apart.
  #[serde(default)]
  pub installs_by_channel: Option<ChannelInstalls>,
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
  /// The channel the release stopped being offered on; Stable from a cloud before Beta pausing.
  Paused {
    #[serde(default)]
    channel: Channel,
  },
  Resumed {
    #[serde(default)]
    channel: Channel,
  },
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
