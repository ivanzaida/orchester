//! Whether an entry is a link: a symbolic link on both platforms, and on Windows also any other reparse point,
//! such as a junction, which `FileType::is_symlink` does not report and which git follows like a folder.

use std::fs::Metadata;

/// The attribute Windows sets on every reparse point (`FILE_ATTRIBUTE_REPARSE_POINT`).
#[cfg(windows)]
const REPARSE_POINT: u32 = 0x400;

/// Whether `entry` (read without following it) is a link.
#[cfg(windows)]
pub(super) fn is_link(entry: &Metadata) -> bool {
  use std::os::windows::fs::MetadataExt;
  entry.file_type().is_symlink() || entry.file_attributes() & REPARSE_POINT != 0
}

/// Whether `entry` (read without following it) is a link.
#[cfg(not(windows))]
pub(super) fn is_link(entry: &Metadata) -> bool {
  entry.file_type().is_symlink()
}
