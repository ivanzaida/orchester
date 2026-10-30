//! The times of a step worktree's checked-out files and folders. Git gives what it checks out the time it wrote it,
//! which is newer than every artifact in the seeded `target/`, so cargo would rebuild each package of the workspace.
//! A file the step checked out holds what the run's same file holds when the run's worktree has no change to it
//! against its `HEAD`, where the step's branch starts: then it takes that file's time, and cargo judges it as it would
//! in the run's worktree. A folder takes the run's folder's time when both hold the same names.

use std::{
  collections::{BTreeSet, HashSet},
  ffi::OsString,
  fs,
  path::Path,
};

use super::times;
use crate::git::Git;

/// Gives each file checked out in `step` that is unchanged in `run`, and each folder of them that holds what the
/// run's does, the time of the run's; how many took it.
pub(super) fn align(run: &Path, step: &Path) -> u64 {
  let changed = match Git::new(run).stdout(&["diff", "--name-only", "--no-renames", "-z", "HEAD"]) {
    Ok(changed) => changed,
    Err(error) => {
      tracing::warn!(%error, "the run's changed files cannot be read; the step's files keep their times");
      return 0;
    }
  };
  let tracked = match Git::new(step).stdout(&["ls-files", "-z"]) {
    Ok(tracked) => tracked,
    Err(error) => {
      tracing::warn!(%error, "the step's files cannot be listed; they keep their times");
      return 0;
    }
  };
  let changed: HashSet<&str> = paths(&changed).collect();
  let files = paths(&tracked)
    .filter(|path| !changed.contains(path))
    .filter(|path| file_takes_time(&run.join(path), &step.join(path)))
    .count();
  let folders: BTreeSet<&Path> = paths(&tracked)
    .flat_map(|path| Path::new(path).ancestors().skip(1))
    .filter(|folder| !folder.as_os_str().is_empty())
    .collect();
  let folders = folders
    .into_iter()
    .filter(|folder| folder_takes_time(&run.join(folder), &step.join(folder)))
    .count();
  u64::try_from(files + folders).unwrap_or(u64::MAX)
}

/// The paths of git's `-z` output.
fn paths(listed: &str) -> impl Iterator<Item = &str> {
  listed.split('\0').filter(|path| !path.is_empty())
}

/// Whether `to`, a plain file as long as `from`, took `from`'s time. A file that cannot keeps the time git gave it,
/// which only makes cargo rebuild its package.
fn file_takes_time(from: &Path, to: &Path) -> bool {
  let (Ok(original), Ok(checked_out)) = (fs::metadata(from), fs::symlink_metadata(to)) else {
    return false;
  };
  if !original.is_file() || !checked_out.is_file() || original.len() != checked_out.len() {
    return false;
  }
  original
    .modified()
    .is_ok_and(|modified| times::set_modified(to, modified).is_ok())
}

/// Whether the folder `to`, which holds the names `from` holds, took `from`'s time. The files in them are judged on
/// their own.
fn folder_takes_time(from: &Path, to: &Path) -> bool {
  let (Some(original), Some(checked_out)) = (names(from), names(to)) else {
    return false;
  };
  if original != checked_out {
    return false;
  }
  fs::metadata(from)
    .and_then(|metadata| metadata.modified())
    .is_ok_and(|modified| times::set_modified(to, modified).is_ok())
}

/// The names in the folder `folder`; `None` when it cannot be read.
fn names(folder: &Path) -> Option<BTreeSet<OsString>> {
  fs::read_dir(folder)
    .ok()?
    .map(|entry| entry.map(|entry| entry.file_name()).ok())
    .collect()
}
