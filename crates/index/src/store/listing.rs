//! The indexes Orchester keeps, as Project settings › Context › Code index lists them: each revision with an
//! index, a running build or a failed one, the runs that use it, and its state. A failed build
//! leaves `.failures/<revision>` with its reason, removed by the next build that succeeds and with
//! the revision's other records once no run uses it.

use std::{collections::BTreeSet, fs, io, path::Path, time::Duration};

use orchester_core::{id::RunId, revision::Revision, time::Timestamp};

use super::{BUILDING, CodebaseIndexes, USERS, locks};
use crate::{
  IndexError,
  manifest::Manifest,
  progress::{self, BuildProgress},
};

pub(super) const FAILURES: &str = ".failures";

/// What a built index holds and what it took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexSummary {
  pub files: usize,
  pub symbols: usize,
  /// The size of its files.
  pub bytes: u64,
  pub built_at: Timestamp,
  pub build_time: Duration,
}

/// Where the index of a revision stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexState {
  Ready(IndexSummary),
  /// Being built (or built again) now.
  Building(BuildProgress),
  /// Its last build failed; `reason` is why.
  Failed {
    reason: String,
  },
  /// Of an earlier format (or with a manifest that does not read): runs that started with it keep
  /// reading it, and the next run from the revision builds it again.
  Outdated,
}

/// One revision of the index list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexListing {
  pub revision: Revision,
  /// The runs whose worktree was created at the revision and that use its index.
  pub runs: Vec<RunId>,
  pub state: IndexState,
}

impl CodebaseIndexes {
  /// The indexes kept for the repository, as the index list shows them: ready ones by when they
  /// were built, newest first, then building, outdated and failed ones.
  pub fn list(&self) -> Result<Vec<IndexListing>, IndexError> {
    let mut listed = Vec::new();
    for revision in self.listed_revisions()? {
      let Some(state) = self.state(&revision)? else {
        continue;
      };
      let runs = names(&self.root.join(USERS).join(revision.as_str()))?
        .iter()
        .filter_map(|name| name.parse().ok())
        .collect();
      listed.push(IndexListing { revision, runs, state });
    }
    listed.sort_by_key(|listing| match &listing.state {
      IndexState::Ready(summary) => (0, -summary.built_at.unix_millis()),
      IndexState::Building(_) => (1, 0),
      IndexState::Outdated => (2, 0),
      IndexState::Failed { .. } => (3, 0),
    });
    Ok(listed)
  }

  /// Revisions with an index folder, a build record or a failure.
  fn listed_revisions(&self) -> Result<BTreeSet<Revision>, IndexError> {
    let mut found = BTreeSet::new();
    for folder in [
      self.root.clone(),
      self.root.join(progress::PROGRESS),
      self.root.join(FAILURES),
    ] {
      for name in names(&folder)? {
        if let Ok(revision) = Revision::new(name.strip_prefix(BUILDING).unwrap_or(&name)) {
          found.insert(revision);
        }
      }
    }
    Ok(found)
  }

  /// The state of `revision`'s index; `None` when there is nothing to show of it. Only records
  /// are read, never the index's folder: a removal renaming it away is not held up, and an index
  /// that went meanwhile is simply not shown.
  fn state(&self, revision: &Revision) -> Result<Option<IndexState>, IndexError> {
    let building = progress::read(&self.root, revision);
    // A build holds the revision's lock exclusively; runs reading the index share it.
    if let Some(progress) = building
      && locks::try_lock_shared(&self.root, revision)?.is_none()
    {
      return Ok(Some(IndexState::Building(progress)));
    }
    let folder = self.root.join(revision.as_str());
    if let Some(manifest) = Manifest::read_current(&folder) {
      return Ok(Some(IndexState::Ready(IndexSummary {
        files: manifest.files,
        symbols: manifest.symbols,
        bytes: manifest.bytes,
        built_at: manifest.built_at,
        build_time: Duration::from_millis(manifest.build_ms),
      })));
    }
    // A failure record that cannot be read shows no failure: the next build writes it again.
    if let Ok(reason) = fs::read_to_string(self.root.join(FAILURES).join(revision.as_str())) {
      return Ok(Some(IndexState::Failed { reason }));
    }
    Ok(folder.is_dir().then_some(IndexState::Outdated))
  }
}

/// Records why the last build of `revision` failed. The index list shows it; a record that
/// cannot be written is logged, as the build's own error reaches its caller either way.
pub(super) fn record_failure(root: &Path, revision: &Revision, reason: &str) {
  let folder = root.join(FAILURES);
  let written = fs::create_dir_all(&folder).and_then(|()| fs::write(folder.join(revision.as_str()), reason));
  if let Err(error) = written {
    tracing::warn!(%error, %revision, "cannot record why the index build failed");
  }
}

/// Removes the failure record of `revision` after a build that succeeded.
pub(super) fn clear_failure(root: &Path, revision: &Revision) {
  let file = root.join(FAILURES).join(revision.as_str());
  if let Err(error) = fs::remove_file(&file)
    && error.kind() != io::ErrorKind::NotFound
  {
    tracing::warn!(%error, file = %file.display(), "cannot remove an old index build failure");
  }
}

/// The entry names of `folder`; none when it does not exist.
fn names(folder: &Path) -> Result<Vec<String>, IndexError> {
  let entries = match fs::read_dir(folder) {
    Ok(entries) => entries,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(error) => return Err(IndexError::io("read", folder)(error)),
  };
  let mut names = Vec::new();
  for entry in entries {
    let entry = entry.map_err(IndexError::io("read", folder))?;
    if let Some(name) = entry.file_name().to_str() {
      names.push(name.to_owned());
    }
  }
  Ok(names)
}
