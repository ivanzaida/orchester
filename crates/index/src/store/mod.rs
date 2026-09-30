//! The indexes of one repository, in `<repo>/.orchester/index/`: one folder per revision a run's
//! worktree was created at. Which runs use a revision is recorded as `.users/<revision>/<run-id>`;
//! an index is kept while one of those runs still has its worktree
//! (`<repo>/.orchester/worktrees/<run-id>`), and removed with the last of them ([`CodebaseIndexes::prune`]).
//!
//! An index is built in `.building-<revision>` beside its final folder and renamed into place once
//! complete, so an agent never reads a half-written one. Each revision has its own lock
//! ([`locks`]): two runs asking for the same revision build it once and the second reuses it, while
//! runs of other revisions never wait for that build, reading an index never waits for another
//! reader, and removal skips a revision that is busy.

mod base;
mod cleanup;
mod listing;
mod locks;

use std::{
  fs::{self, File},
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{id::RunId, integration::WORKTREES_DIR, revision::Revision};

pub use self::listing::{IndexListing, IndexState, IndexSummary};
use crate::{
  IndexError,
  build::{self, Base},
  digest,
  manifest::{BuildMode, Manifest},
  progress::ProgressFile,
  write,
};

const USERS: &str = ".users";
const BUILDING: &str = ".building-";

/// Where a build starts.
#[derive(Clone, Copy)]
enum Start {
  /// From the existing index closest to the revision, when there is one.
  Closest,
  /// From nothing.
  Scratch,
}

/// A build that happened.
#[derive(Clone, Debug)]
pub struct BuildReport {
  pub mode: BuildMode,
  pub duration: Duration,
  /// Tracked files in the index.
  pub files: usize,
  /// Files whose content was read and indexed.
  pub read: usize,
  /// Files taken over from the earlier index unchanged.
  pub reused: usize,
}

#[derive(Clone, Debug)]
pub enum Outcome {
  /// The index already existed.
  Reused,
  Built(BuildReport),
}

/// The index of a revision, ready to read.
#[derive(Clone, Debug)]
pub struct Ensured {
  pub path: PathBuf,
  pub outcome: Outcome,
}

/// The codebase indexes of one repository.
#[derive(Clone, Debug)]
pub struct CodebaseIndexes {
  repo: PathBuf,
  root: PathBuf,
}

impl CodebaseIndexes {
  pub fn new(repo: &Path) -> Self {
    Self {
      repo: repo.to_path_buf(),
      root: repo.join(".orchester").join("index"),
    }
  }

  /// The index of `revision` for `run`, whose worktree was created at it: reused when it exists,
  /// otherwise built, from the existing index closest to it when there is one. Indexes whose runs
  /// no longer have worktrees are removed afterwards. Reuse holds the revision's shared lock, so it
  /// does not wait for another revision's build that reads this index as its base.
  pub fn ensure(&self, revision: &Revision, run: RunId) -> Result<Ensured, IndexError> {
    fs::create_dir_all(&self.root).map_err(IndexError::io("create", &self.root))?;
    let path = self.root.join(revision.as_str());
    let reused = {
      let _lock = locks::lock_shared(&self.root, revision)?;
      let current =
        Manifest::read_current(&path).is_some_and(|manifest| digest::verify(&path, &manifest.digests).is_ok());
      if current {
        self.record_user(revision, run)?;
      }
      current
    };
    let outcome = if reused {
      Outcome::Reused
    } else {
      let _lock = locks::lock(&self.root, revision)?;
      self.record_user(revision, run)?;
      // Another run may have built it while this one waited for the lock.
      match Manifest::read_current(&path).map(|manifest| digest::verify(&path, &manifest.digests)) {
        Some(Ok(())) => Outcome::Reused,
        Some(Err(problem)) => {
          tracing::warn!(%revision, %problem, "the codebase index was changed; building it again");
          Outcome::Built(self.build(revision, &path, Start::Scratch)?)
        }
        None => Outcome::Built(self.build(revision, &path, Start::Closest)?),
      }
    };
    if let Err(error) = self.prune() {
      tracing::warn!(%error, "cannot remove the codebase indexes no run uses");
    }
    Ok(Ensured { path, outcome })
  }

  /// Records that `run` uses the index of `revision`, while holding its lock: a removal, which
  /// takes it exclusively, sees the record.
  fn record_user(&self, revision: &Revision, run: RunId) -> Result<(), IndexError> {
    let users = self.root.join(USERS).join(revision.as_str());
    fs::create_dir_all(&users).map_err(IndexError::io("create", &users))?;
    let user = users.join(run.to_string());
    File::create(&user).map_err(IndexError::io("create", user))?;
    Ok(())
  }

  /// Removes the indexes of revisions whose runs all had their worktrees removed. It never waits:
  /// a revision being built or reused is left for the next removal.
  pub fn prune(&self) -> Result<(), IndexError> {
    if !self.root.is_dir() {
      return Ok(());
    }
    cleanup::remove_unused(&self.root, &self.repo.join(WORKTREES_DIR))
  }

  /// Builds the index of `revision` again from scratch, replacing the one there is, if any (the
  /// index list's *Rebuild* and *Retry*). The runs that use it keep using it; a run started from
  /// the revision uses the new one.
  pub fn rebuild(&self, revision: &Revision) -> Result<BuildReport, IndexError> {
    fs::create_dir_all(&self.root).map_err(IndexError::io("create", &self.root))?;
    let _lock = locks::lock(&self.root, revision)?;
    self.build(revision, &self.root.join(revision.as_str()), Start::Scratch)
  }

  /// The folder of the index of `revision`, when it is built and unchanged.
  pub fn existing(&self, revision: &Revision) -> Option<PathBuf> {
    let path = self.root.join(revision.as_str());
    // Without the lock a build or removal may be swapping the folder: the index is not trusted.
    let _lock = locks::try_lock_shared(&self.root, revision).ok().flatten()?;
    let manifest = Manifest::read_current(&path)?;
    match digest::verify(&path, &manifest.digests) {
      Ok(()) => Some(path),
      Err(problem) => {
        tracing::warn!(%revision, %problem, "the codebase index was changed");
        None
      }
    }
  }

  /// Builds the index of `revision` into `path`, holding the revision's lock, and records how it
  /// went for the index list.
  fn build(&self, revision: &Revision, path: &Path, start: Start) -> Result<BuildReport, IndexError> {
    let progress = ProgressFile::start(&self.root, revision);
    let built = self.publish(revision, path, start, &progress);
    match &built {
      Ok(_) => listing::clear_failure(&self.root, revision),
      Err(error) => listing::record_failure(&self.root, revision, &error.to_string()),
    }
    built
  }

  fn publish(
    &self,
    revision: &Revision,
    path: &Path,
    start: Start,
    progress: &ProgressFile,
  ) -> Result<BuildReport, IndexError> {
    let staging = self.root.join(format!("{BUILDING}{revision}"));
    // The earlier index stays locked (shared) while this build reads it: a removal leaves it alone.
    let (base, _base_lock) = match start {
      Start::Closest => self.base(revision).unzip(),
      Start::Scratch => (None, None),
    };
    let built = match self.build_into(revision, base.as_ref(), (&staging, progress)) {
      Err(error @ IndexError::Previous { .. }) => {
        tracing::warn!(%error, "building the codebase index in full instead");
        self.build_into(revision, None, (&staging, progress))?
      }
      other => other?,
    };
    write::make_read_only(&staging).map_err(IndexError::io("seal", &staging))?;
    // The index there was, of an earlier format or rebuilt, is replaced.
    cleanup::discard(&self.root, path)?;
    fs::rename(&staging, path).map_err(IndexError::io("rename", &staging))?;
    Ok(BuildReport {
      mode: built.manifest.build,
      duration: Duration::from_millis(built.manifest.build_ms),
      files: built.manifest.files,
      read: built.read,
      reused: built.reused,
    })
  }

  fn build_into(
    &self,
    revision: &Revision,
    base: Option<&Base>,
    (staging, progress): (&Path, &ProgressFile),
  ) -> Result<build::Built, IndexError> {
    cleanup::discard(&self.root, staging)?;
    fs::create_dir(staging).map_err(IndexError::io("create", staging))?;
    let built = build::build(&self.repo, revision, base, (staging, progress));
    if built.is_err()
      && let Err(error) = cleanup::discard(&self.root, staging)
    {
      tracing::warn!(%error, "cannot remove a codebase index that was not finished");
    }
    built
  }

  /// The revisions with an index folder. One that cannot be listed is no index to start from; a
  /// full build does not need it.
  fn revisions(&self) -> Vec<Revision> {
    let Ok(entries) = fs::read_dir(&self.root) else {
      return Vec::new();
    };
    entries
      .filter_map(|entry| Revision::new(entry.ok()?.file_name().to_str()?).ok())
      .collect()
  }
}
