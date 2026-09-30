//! The indexes of one repository, in `<repo>/.orchester/index/`: one folder per revision a run's
//! worktree was created at. Which runs use a revision is recorded as `.users/<revision>/<run-id>`;
//! an index is kept while one of those runs still has its worktree
//! (`<repo>/.orchester/worktrees/<run-id>`), and removed with the last of them ([`CodebaseIndexes::prune`]).
//!
//! An index is built in `.building-<revision>` beside its final folder and renamed into place once
//! complete, so an agent never reads a half-written one. Each revision has its own lock
//! ([`locks`]): two runs asking for the same revision build it once and the second reuses it, while
//! runs of other revisions never wait for that build, and removal skips a revision that is busy.

mod cleanup;
mod locks;

use std::{
  fs::{self, File},
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{id::RunId, integration::WORKTREES_DIR, revision::Revision};

use crate::{
  IndexError,
  build::{self, Base},
  git::Repository,
  manifest::{BuildMode, Manifest},
  write,
};

const USERS: &str = ".users";
const BUILDING: &str = ".building-";

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
  /// no longer have worktrees are removed afterwards.
  pub fn ensure(&self, revision: &Revision, run: RunId) -> Result<Ensured, IndexError> {
    fs::create_dir_all(&self.root).map_err(IndexError::io("create", &self.root))?;
    let lock = locks::lock(&self.root, revision)?;
    let users = self.root.join(USERS).join(revision.as_str());
    fs::create_dir_all(&users).map_err(IndexError::io("create", &users))?;
    let user = users.join(run.to_string());
    File::create(&user).map_err(IndexError::io("create", user))?;
    let path = self.root.join(revision.as_str());
    let outcome = if Manifest::read_current(&path).is_some() {
      Outcome::Reused
    } else {
      Outcome::Built(self.build(revision, &path)?)
    };
    drop(lock);
    if let Err(error) = self.prune() {
      tracing::warn!(%error, "cannot remove the codebase indexes no run uses");
    }
    Ok(Ensured { path, outcome })
  }

  /// Removes the indexes of revisions whose runs all had their worktrees removed. It never waits:
  /// a revision being built or reused is left for the next removal.
  pub fn prune(&self) -> Result<(), IndexError> {
    if !self.root.is_dir() {
      return Ok(());
    }
    cleanup::remove_unused(&self.root, &self.repo.join(WORKTREES_DIR))
  }

  fn build(&self, revision: &Revision, path: &Path) -> Result<BuildReport, IndexError> {
    let staging = self.root.join(format!("{BUILDING}{revision}"));
    let base = self.base(revision);
    let built = match self.build_into(revision, base.as_ref(), &staging) {
      Err(error @ IndexError::Previous { .. }) => {
        tracing::warn!(%error, "building the codebase index in full instead");
        self.build_into(revision, None, &staging)?
      }
      other => other?,
    };
    write::make_read_only(&staging).map_err(IndexError::io("seal", &staging))?;
    // An index of an earlier format is replaced.
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

  fn build_into(&self, revision: &Revision, base: Option<&Base>, staging: &Path) -> Result<build::Built, IndexError> {
    cleanup::discard(&self.root, staging)?;
    fs::create_dir(staging).map_err(IndexError::io("create", staging))?;
    let built = build::build(&self.repo, revision, base, staging);
    if built.is_err()
      && let Err(error) = cleanup::discard(&self.root, staging)
    {
      tracing::warn!(%error, "cannot remove a codebase index that was not finished");
    }
    built
  }

  /// The existing index an incremental build of `revision` starts from: the one with the fewest
  /// changed paths. A revision git cannot compare with is passed over.
  fn base(&self, revision: &Revision) -> Option<Base> {
    let git = Repository::new(&self.repo);
    self
      .revisions()
      .into_iter()
      .filter(|existing| existing != revision && Manifest::read_current(&self.root.join(existing.as_str())).is_some())
      .filter_map(|existing| match git.changed_paths(&existing, revision) {
        Ok(changed) => Some(Base {
          dir: self.root.join(existing.as_str()),
          revision: existing,
          changed,
        }),
        Err(error) => {
          tracing::warn!(%error, from = %existing, "cannot compare with an existing codebase index");
          None
        }
      })
      .min_by(|left, right| (left.changed.len(), &left.revision).cmp(&(right.changed.len(), &right.revision)))
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
