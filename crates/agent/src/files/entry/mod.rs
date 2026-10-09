//! Deleting and renaming one directory entry of the worktree exactly as it was checked (`delete_file`, `move_file`):
//! the entry is reached through handles that do not follow links, so a link or junction swapped in after the check
//! cannot send the change elsewhere, and a link at the end of the path is deleted or moved itself, never its target.
//! On macOS (and other Unix systems) the folders from the worktree down are opened one by one without following links
//! and the entry is changed relative to its folder ([`unix`]); on Windows the entry is opened without following a link
//! at its end, the path it opened is read back from the handle and must be the one checked, and the change goes
//! through that handle ([`windows`]). A rename never replaces an entry that is already at the target.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::{io, path::Path};

#[cfg(unix)]
use self::unix as platform;
#[cfg(windows)]
use self::windows as platform;

/// Why an entry was not changed.
#[derive(Debug)]
pub enum EntryError {
  /// The path no longer leads where it was checked to: a link or junction lies on the way.
  Moved,
  /// The entry is a folder, which these tools never delete or move.
  Folder,
  /// An entry is already at the rename's target.
  Exists,
  Io(io::Error),
}

impl From<io::Error> for EntryError {
  fn from(error: io::Error) -> Self {
    match error.kind() {
      io::ErrorKind::AlreadyExists => Self::Exists,
      _ => Self::Io(error),
    }
  }
}

/// Deletes the entry at `path`, inside the folder `root` (both resolved, `path`'s last name not followed).
///
/// Public only so the tests can put a link in place before the entry is reached; callers must check the step's scope
/// first, as `FileTools::delete_file` does: this only refuses what changed since that check.
///
/// # Errors
///
/// [`EntryError::Moved`] when a link or junction on the way leads elsewhere than `path`, [`EntryError::Folder`] for a
/// folder, [`EntryError::Io`] when the system refuses.
#[doc(hidden)]
pub fn remove(root: &Path, path: &Path) -> Result<(), EntryError> {
  platform::remove(root, path)
}

/// Renames the entry at `from` to `to`, where no entry may be; all inside `root` and resolved but their last names.
///
/// Public only so the tests can put a link in place before the target's folder is opened; callers must check the step's
/// scope first, as `FileTools::move_file` does: this only refuses what changed since that check.
///
/// # Errors
///
/// [`EntryError::Moved`] when a link or junction on the way to either leads elsewhere, the target's folder included,
/// [`EntryError::Folder`] for a folder, [`EntryError::Exists`] when an entry is at `to`, [`EntryError::Io`] when the
/// system refuses.
#[doc(hidden)]
pub fn rename(root: &Path, from: &Path, to: &Path) -> Result<(), EntryError> {
  platform::rename(root, from, to)
}
