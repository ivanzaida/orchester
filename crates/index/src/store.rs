//! The indexes of one repository, in `<repo>/.orchester/index/`: one folder per revision a run's
//! worktree was created at. Which runs use a revision is recorded as `.users/<revision>/<run-id>`;
//! an index is kept while one of those runs still has its worktree
//! (`<repo>/.orchester/worktrees/<run-id>`), and removed with the last of them ([`CodebaseIndexes::prune`]).
//!
//! An index is built in `.building-<revision>` beside its final folder and renamed into place once
//! complete, so an agent never reads a half-written one. Building, reusing and removing hold an
//! exclusive lock on `.lock`, in this process and across processes (the runner builds, the desktop
//! removes): two runs asking for the same revision build it once, the second reuses it.

use std::{
  collections::HashSet,
  fs::{self, File, OpenOptions},
  io,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{id::RunId, integration::WORKTREES_DIR, revision::Revision};

use crate::{
  IndexError,
  build::{self, Base},
  git::Repository,
  manifest::{BuildMode, Manifest},
};

const INDEX_DIR: &str = ".orchester/index";
const USERS: &str = ".users";
const BUILDING: &str = ".building-";
const LOCK: &str = ".lock";

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
      root: repo.join(INDEX_DIR),
    }
  }

  /// The index of `revision` for `run`, whose worktree was created at it: reused when it exists,
  /// otherwise built, from the existing index closest to it when there is one. Indexes whose runs
  /// no longer have worktrees are removed afterwards.
  pub fn ensure(&self, revision: &Revision, run: RunId) -> Result<Ensured, IndexError> {
    fs::create_dir_all(&self.root).map_err(IndexError::io("create", &self.root))?;
    let _lock = self.lock()?;
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
    if let Err(error) = self.remove_unused() {
      tracing::warn!(%error, "cannot remove the codebase indexes no run uses");
    }
    Ok(Ensured { path, outcome })
  }

  /// Removes the indexes of revisions whose runs all had their worktrees removed.
  pub fn prune(&self) -> Result<(), IndexError> {
    if !self.root.is_dir() {
      return Ok(());
    }
    let _lock = self.lock()?;
    self.remove_unused()
  }

  /// Holds the lock until the returned file is dropped.
  fn lock(&self) -> Result<File, IndexError> {
    let path = self.root.join(LOCK);
    let file = OpenOptions::new()
      .create(true)
      .truncate(false)
      .write(true)
      .open(&path)
      .map_err(IndexError::io("open", &path))?;
    file.lock().map_err(IndexError::io("lock", path))?;
    Ok(file)
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
    // An index of an earlier format is replaced.
    remove_folder(path)?;
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
    remove_folder(staging)?;
    fs::create_dir(staging).map_err(IndexError::io("create", staging))?;
    let built = build::build(&self.repo, revision, base, staging);
    if built.is_err()
      && let Err(error) = remove_folder(staging)
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

  /// Removes the records of runs whose worktree is gone, then every index no run uses and any build
  /// left unfinished.
  fn remove_unused(&self) -> Result<(), IndexError> {
    let used = self.used_revisions()?;
    let entries = fs::read_dir(&self.root).map_err(IndexError::io("read", &self.root))?;
    for entry in entries {
      let entry = entry.map_err(IndexError::io("read", &self.root))?;
      let name = entry.file_name();
      let Some(name) = name.to_str() else {
        continue;
      };
      let unused = Revision::new(name).is_ok_and(|revision| !used.contains(&revision));
      if unused || name.starts_with(BUILDING) {
        remove_folder(&entry.path())?;
      }
    }
    Ok(())
  }

  /// The revisions whose index a run with a worktree uses; records of the others are removed.
  fn used_revisions(&self) -> Result<HashSet<Revision>, IndexError> {
    let users = self.root.join(USERS);
    let worktrees = self.repo.join(WORKTREES_DIR);
    let mut used = HashSet::new();
    for revision in read_folder(&users)? {
      let mut in_use = false;
      for run in read_folder(&revision)? {
        let name = run.file_name().expect("a folder entry has a name");
        if worktrees.join(name).is_dir() {
          in_use = true;
        } else {
          fs::remove_file(&run).map_err(IndexError::io("remove", &run))?;
        }
      }
      let name = revision.file_name().and_then(|name| name.to_str());
      match name.map(Revision::new) {
        Some(Ok(revision)) if in_use => {
          used.insert(revision);
        }
        _ => remove_folder(&revision)?,
      }
    }
    Ok(used)
  }
}

/// The entries of `folder`; none when it does not exist.
fn read_folder(folder: &Path) -> Result<Vec<PathBuf>, IndexError> {
  match fs::read_dir(folder) {
    Ok(entries) => entries
      .map(|entry| entry.map(|entry| entry.path()))
      .collect::<io::Result<_>>()
      .map_err(IndexError::io("read", folder)),
    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
    Err(error) => Err(IndexError::io("read", folder)(error)),
  }
}

fn remove_folder(folder: &Path) -> Result<(), IndexError> {
  match fs::remove_dir_all(folder) {
    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(IndexError::io("remove", folder)(error)),
    _ => Ok(()),
  }
}
