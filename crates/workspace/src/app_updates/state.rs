//! Where this copy's update stands, as Settings › Updates and the status bar show it: one value for each row of the
//! design's update row (`crates/desktop-app/design/orchester.pen`, `c69Eti`).

use std::path::PathBuf;

use orchester_api::releases::{ReleaseVersion, SignatureError};
use orchester_core::time::Timestamp;

use super::{
  copy::CopyReason,
  install::{InstallFailure, InstallStep},
};

/// The update row's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateState {
  /// Orchester Cloud offers nothing newer for this copy's channel.
  UpToDate { checked_at: Timestamp },
  /// Asking Orchester Cloud for the newest release of this copy's channel.
  Checking,
  /// Downloading `version` into the updates folder; resumed where an interrupted download stopped.
  Downloading {
    version: ReleaseVersion,
    received: u64,
    total: u64,
  },
  /// A newer release was downloaded and checked against its SHA-256 and its signature; it installs once no run is at
  /// work, or when the person says so.
  Ready(ReadyUpdate),
  /// A ready update waits for the runs at work to end.
  WaitingForRuns {
    update: ReadyUpdate,
    /// The runs at work, as the person knows them (`ORC-110`).
    runs: Vec<String>,
  },
  /// The person started installing `update` (*Restart to update*); the window closes once the installer took over.
  Installing { update: ReadyUpdate, step: InstallStep },
  /// `version` could not be made ready; nothing of it was kept or run.
  Failed {
    version: ReleaseVersion,
    failure: UpdateFailure,
  },
  /// `version` was ready but did not install: this version keeps running. It stays until another release is offered.
  InstallFailed {
    version: ReleaseVersion,
    failure: InstallFailure,
  },
  /// Orchester Cloud did not answer the last check, or broke off the download; it is asked again at the next check.
  CloudUnreachable { last_checked: Option<Timestamp> },
  /// This copy does not update itself; the newest release, once the cloud named one, is downloaded by hand.
  NotThisCopy {
    reason: CopyReason,
    newest: Option<NewestRelease>,
  },
  /// Updates are not automatic on this platform yet (macOS); the newest release is downloaded by hand.
  NotOnThisPlatform { newest: Option<NewestRelease> },
}

impl UpdateState {
  /// The state with `runs` at work: a ready update waits for them, and runs nobody waits for change nothing.
  pub(super) fn with_runs(self, runs: &[String]) -> Self {
    match self {
      Self::Ready(update) | Self::WaitingForRuns { update, .. } if !runs.is_empty() => Self::WaitingForRuns {
        update,
        runs: runs.to_vec(),
      },
      Self::WaitingForRuns { update, .. } => Self::Ready(update),
      state => state,
    }
  }

  /// The downloaded and verified update, ready or waiting for runs.
  pub fn ready(&self) -> Option<&ReadyUpdate> {
    match self {
      Self::Ready(update) | Self::WaitingForRuns { update, .. } => Some(update),
      _ => None,
    }
  }

  /// The newest release a copy that does not update itself was told of.
  pub(super) fn newest(&self) -> Option<&NewestRelease> {
    match self {
      Self::NotThisCopy { newest, .. } | Self::NotOnThisPlatform { newest } => newest.as_ref(),
      _ => None,
    }
  }

  /// How far a download has come, as a whole percentage; `None` outside one.
  pub fn percent(&self) -> Option<u8> {
    let Self::Downloading { received, total, .. } = self else {
      return None;
    };
    let percent = (*received).min(*total) * 100 / (*total).max(1);
    Some(u8::try_from(percent).expect("at most 100"))
  }
}

/// A downloaded and verified update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadyUpdate {
  pub version: ReleaseVersion,
  /// Release notes in Markdown.
  pub notes: String,
  /// It fixes a security problem: it installs at the next restart, runs at work still waited for.
  pub critical: bool,
  /// The verified installer in the updates folder.
  pub installer: PathBuf,
}

/// A release newer than this copy, for a copy that is updated by hand: its version and where its file downloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewestRelease {
  pub version: ReleaseVersion,
  pub download: String,
}

/// Why an update could not be made ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateFailure {
  /// The release has no installer for this platform and architecture.
  NoInstaller,
  /// The downloaded file is not the size the release names; it was deleted.
  WrongSize,
  /// The downloaded file's SHA-256 is not the one the release names; it was deleted.
  WrongDigest,
  /// The file was signed with a key this app does not accept; it was deleted.
  UnknownKey,
  /// The file's signature does not verify with the key it names; it was deleted.
  BadSignature,
  /// The updates folder could not be written or read; the log says why.
  NotSaved,
}

impl From<SignatureError> for UpdateFailure {
  fn from(error: SignatureError) -> Self {
    match error {
      SignatureError::UnknownKey => Self::UnknownKey,
      SignatureError::BadSignature => Self::BadSignature,
    }
  }
}
