//! The disk space of the repository's disk, for the disk guard ([`orchester_core::workspaces::DiskSpace`]), and the
//! size of folders on it (workspaces, warm folders, old worktrees). Reading the free space is platform code:
//! `GetDiskFreeSpaceExW` on Windows ([`windows`]), `statvfs` on macOS and other Unix systems ([`unix`]).
//!
//! A debug build reads [`FREE_SPACE_OVERRIDE`] first, so a QA copy can show a low disk without filling one.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use orchester_core::workspaces::{DiskSpace, DiskSpaceError, DiskUsage, GIGABYTE};

/// In a debug build, gigabytes free that every disk reads as (`92`), or `<free>/<total>` (`92/1800`).
pub const FREE_SPACE_OVERRIDE: &str = "ORCHESTER_FREE_SPACE_OVERRIDE_GB";

/// The disks of this computer, as the operating system reports them.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalDisks;

impl LocalDisks {
  /// The space of the disk `folder` is on as the operating system reports it, whatever [`FREE_SPACE_OVERRIDE`] says.
  pub fn measured(folder: &Path) -> Result<DiskUsage, DiskSpaceError> {
    let existing = existing_ancestor(folder);
    platform_usage(&existing).map_err(|error| DiskSpaceError::Unreadable {
      path: folder.to_path_buf(),
      detail: error.to_string(),
    })
  }
}

impl DiskSpace for LocalDisks {
  fn usage(&self, folder: &Path) -> Result<DiskUsage, DiskSpaceError> {
    overridden().map_or_else(|| Self::measured(folder), Ok)
  }
}

#[cfg(windows)]
fn platform_usage(folder: &Path) -> io::Result<DiskUsage> {
  windows::usage(folder)
}

#[cfg(unix)]
fn platform_usage(folder: &Path) -> io::Result<DiskUsage> {
  unix::usage(folder)
}

/// `folder`, or the nearest folder above it that exists: the free space of a folder still to be made is its disk's.
fn existing_ancestor(folder: &Path) -> PathBuf {
  folder
    .ancestors()
    .find(|candidate| candidate.is_dir())
    .unwrap_or(folder)
    .to_path_buf()
}

/// What [`FREE_SPACE_OVERRIDE`] says in a debug build; `None` in a release build, or when it is not set or unreadable.
fn overridden() -> Option<DiskUsage> {
  if !cfg!(debug_assertions) {
    return None;
  }
  let value = std::env::var(FREE_SPACE_OVERRIDE).ok()?;
  let (free, total) = value.split_once('/').unwrap_or((&value, "2000"));
  let free: u64 = free.trim().parse().ok()?;
  let total: u64 = total.trim().parse().ok()?;
  Some(DiskUsage {
    free: free.saturating_mul(GIGABYTE),
    total: total.max(free).saturating_mul(GIGABYTE),
  })
}

/// The bytes of every file under `folder`, which links are not followed into; 0 for a folder that is not there. A
/// file removed while it is read counts as gone.
pub fn folder_size(folder: &Path) -> io::Result<u64> {
  let mut total = 0_u64;
  let mut pending = vec![folder.to_path_buf()];
  while let Some(current) = pending.pop() {
    let entries = match fs::read_dir(&current) {
      Ok(entries) => entries,
      Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
      Err(error) => return Err(error),
    };
    for entry in entries {
      let entry = entry?;
      let metadata = match entry.path().symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
        Err(error) => return Err(error),
      };
      if metadata.is_dir() {
        pending.push(entry.path());
      } else if metadata.is_file() {
        total = total.saturating_add(metadata.len());
      }
    }
  }
  Ok(total)
}
