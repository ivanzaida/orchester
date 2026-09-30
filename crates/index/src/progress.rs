//! What a build is doing, for a person watching the index list: `.progress/<revision>` in the index
//! folder, written when the build moves to its next phase and removed when it ends. Each phase is
//! written beside it and renamed over it, so a reader never sees half a record. Only a build whose
//! revision lock is held counts as running: a file a crashed build left says nothing.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{revision::Revision, time::Timestamp};
use serde::{Deserialize, Serialize};

pub(crate) const PROGRESS: &str = ".progress";
/// The extension of a record being written.
const STAGED: &str = "writing";

/// The phase a build is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BuildPhase {
  /// Listing the revision's tracked files.
  Listing,
  /// Reading and parsing `files` tracked files.
  Indexing { files: usize },
  /// Writing the index files and the package map.
  Writing,
}

/// A running build: when it started and what it does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildProgress {
  pub started_at: Timestamp,
  pub phase: BuildPhase,
}

/// Records the phases of one build of a revision.
pub(crate) struct ProgressFile {
  path: PathBuf,
  started_at: Timestamp,
}

impl ProgressFile {
  pub(crate) fn start(root: &Path, revision: &Revision) -> Self {
    let progress = Self {
      path: root.join(PROGRESS).join(revision.as_str()),
      started_at: Timestamp::now(),
    };
    progress.phase(BuildPhase::Listing);
    progress
  }

  /// Records `phase`. What a person sees of a build is no reason to fail it: a record that cannot
  /// be written is logged.
  pub(crate) fn phase(&self, phase: BuildPhase) {
    let progress = BuildProgress {
      started_at: self.started_at,
      phase,
    };
    let text = serde_json::to_string(&progress).expect("progress serializes to JSON");
    let staged = self.path.with_extension(STAGED);
    let written = self
      .path
      .parent()
      .map_or(Ok(()), fs::create_dir_all)
      .and_then(|()| fs::write(&staged, text))
      .and_then(|()| fs::rename(&staged, &self.path));
    if let Err(error) = written {
      tracing::debug!(%error, path = %self.path.display(), "cannot record the index build's progress");
    }
  }
}

impl Drop for ProgressFile {
  fn drop(&mut self) {
    if let Err(error) = fs::remove_file(&self.path)
      && error.kind() != std::io::ErrorKind::NotFound
    {
      tracing::debug!(%error, path = %self.path.display(), "cannot remove the index build's progress");
    }
  }
}

/// The progress recorded for `revision`, if a build recorded one.
pub(crate) fn read(root: &Path, revision: &Revision) -> Option<BuildProgress> {
  // A record that cannot be read shows no progress; the build itself goes on.
  let text = fs::read_to_string(root.join(PROGRESS).join(revision.as_str())).ok()?;
  serde_json::from_str(&text).ok()
}
