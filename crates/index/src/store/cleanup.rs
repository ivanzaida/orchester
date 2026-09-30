//! Removing the indexes no run uses: a revision's records of runs whose worktree is gone are
//! deleted, then its index and any build of it left unfinished, while no one holds its lock; a
//! revision whose lock is held (it is being built or reused) is left for the next removal. A folder
//! is renamed into `.trash-*` before it is deleted, so a deletion that stops halfway (a file another
//! program holds) never leaves something that reads as an index; what stays in the trash goes with
//! the next removal.

use std::{
  collections::BTreeSet,
  fs, io,
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
};

use orchester_core::{revision::Revision, time::Timestamp};

use super::{BUILDING, USERS, listing::FAILURES, locks};
use crate::{IndexError, progress::PROGRESS};

const TRASH: &str = ".trash-";

/// Numbers the trash folders of this process.
static DISCARDED: AtomicU64 = AtomicU64::new(0);

/// Removes the indexes in `root` whose runs no longer have their worktree in `worktrees`.
pub(super) fn remove_unused(root: &Path, worktrees: &Path) -> Result<(), IndexError> {
  for revision in revisions(root)? {
    let Some(_lock) = locks::try_lock(root, &revision)? else {
      continue;
    };
    if !used(root, worktrees, &revision)? {
      discard(root, &root.join(revision.as_str()))?;
      discard(root, &root.join(format!("{BUILDING}{revision}")))?;
      for record in [FAILURES, PROGRESS] {
        remove_file(&root.join(record).join(revision.as_str()))?;
      }
    }
  }
  for entry in read_folder(root)? {
    let trash = entry
      .file_name()
      .and_then(|name| name.to_str())
      .is_some_and(|name| name.starts_with(TRASH));
    if trash && let Err(error) = remove_folder(&entry) {
      tracing::warn!(%error, folder = %entry.display(), "cannot empty the codebase index trash yet");
    }
  }
  Ok(())
}

/// Every revision with an index, a build or run records in `root`.
fn revisions(root: &Path) -> Result<BTreeSet<Revision>, IndexError> {
  let names = read_folder(root)?
    .into_iter()
    .chain(read_folder(&root.join(USERS))?)
    .chain(read_folder(&root.join(FAILURES))?)
    .filter_map(|path| path.file_name().and_then(|name| name.to_str()).map(str::to_owned));
  Ok(
    names
      .filter_map(|name| Revision::new(name.strip_prefix(BUILDING).unwrap_or(&name)).ok())
      .collect(),
  )
}

/// Whether a run with a worktree uses the index of `revision`; the records of the others are
/// deleted.
fn used(root: &Path, worktrees: &Path, revision: &Revision) -> Result<bool, IndexError> {
  let users = root.join(USERS).join(revision.as_str());
  let mut in_use = false;
  for run in read_folder(&users)? {
    let name = run.file_name().expect("a folder entry has a name");
    if worktrees.join(name).is_dir() {
      in_use = true;
    } else {
      fs::remove_file(&run).map_err(IndexError::io("remove", &run))?;
    }
  }
  if !in_use {
    remove_folder(&users).map_err(IndexError::io("remove", &users))?;
  }
  Ok(in_use)
}

/// Moves `folder` of the index folder `root` into the trash and deletes it there; a deletion that
/// fails is logged and retried by the next removal. [`IndexError::InUse`] when a program holds one
/// of its files open.
pub(super) fn discard(root: &Path, folder: &Path) -> Result<(), IndexError> {
  let number = DISCARDED.fetch_add(1, Ordering::Relaxed);
  // The time tells this process's trash from what an earlier process with the same id left.
  let started = Timestamp::now().unix_millis();
  let trash = root.join(format!("{TRASH}{}-{started}-{number}", std::process::id()));
  match fs::rename(folder, &trash) {
    Ok(()) => {}
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
    Err(error) if held_open(&error) => {
      let revision = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
      return Err(IndexError::InUse { revision });
    }
    Err(error) => return Err(IndexError::io("move to the trash", folder)(error)),
  }
  if let Err(error) = remove_folder(&trash) {
    tracing::warn!(%error, folder = %trash.display(), "cannot delete a discarded codebase index yet");
  }
  Ok(())
}

/// Whether a folder could not be renamed because a program holds one of its files open: Windows
/// refuses it (access denied, sharing violation).
#[cfg(windows)]
fn held_open(error: &io::Error) -> bool {
  const ACCESS_DENIED: i32 = 5;
  const SHARING_VIOLATION: i32 = 32;
  matches!(error.raw_os_error(), Some(ACCESS_DENIED | SHARING_VIOLATION))
}

/// Unix renames a folder whose files are open: that never fails a removal.
#[cfg(not(windows))]
fn held_open(_error: &io::Error) -> bool {
  false
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

fn remove_folder(folder: &Path) -> io::Result<()> {
  match fs::remove_dir_all(folder) {
    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
    _ => Ok(()),
  }
}

fn remove_file(file: &Path) -> Result<(), IndexError> {
  match fs::remove_file(file) {
    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(IndexError::io("remove", file)(error)),
    _ => Ok(()),
  }
}
