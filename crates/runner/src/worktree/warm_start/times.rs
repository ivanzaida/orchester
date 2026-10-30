//! Setting a checked-out file's or folder's time. A folder's time counts too: cargo reruns a build script whose
//! `rerun-if-changed` names a folder when anything in it, the folder itself included, is newer than its last run.
//! Windows opens a folder only with backup semantics, and changing its time needs no more than the right to write its
//! attributes; on macOS and Linux the owner sets the times of anything it opens.

use std::{fs, io, path::Path, time::SystemTime};

/// Gives the file or folder at `path` the time `modified`.
pub(super) fn set_modified(path: &Path, modified: SystemTime) -> io::Result<()> {
  open(path)?.set_modified(modified)
}

#[cfg(windows)]
fn open(path: &Path) -> io::Result<fs::File> {
  use std::os::windows::fs::OpenOptionsExt;

  // `FILE_WRITE_ATTRIBUTES` and `FILE_FLAG_BACKUP_SEMANTICS` (winnt.h, winbase.h).
  const WRITE_ATTRIBUTES: u32 = 0x0100;
  const BACKUP_SEMANTICS: u32 = 0x0200_0000;
  fs::OpenOptions::new()
    .access_mode(WRITE_ATTRIBUTES)
    .custom_flags(BACKUP_SEMANTICS)
    .open(path)
}

#[cfg(not(windows))]
fn open(path: &Path) -> io::Result<fs::File> {
  fs::File::open(path)
}
