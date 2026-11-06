use std::{fs, os::unix::fs::MetadataExt, path::Path};

use super::Stamp;

/// The stamp of the entry at `path`, its link if it is one; none when it cannot be read, which only means it is hashed.
pub(super) fn stamp(path: &Path) -> Option<Stamp> {
  let metadata = fs::symlink_metadata(path).ok()?;
  Some(Stamp {
    size: metadata.size(),
    modified: time(metadata.mtime(), metadata.mtime_nsec()),
    changed: time(metadata.ctime(), metadata.ctime_nsec()),
    identity: (metadata.dev(), metadata.ino()),
    kind: metadata.mode(),
  })
}

fn time(seconds: i64, nanoseconds: i64) -> i128 {
  i128::from(seconds) * 1_000_000_000 + i128::from(nanoseconds)
}
