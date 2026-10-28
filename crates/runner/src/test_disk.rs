//! The disk a runner built with the tests' stand-in agents (`test-agent`) reads for its disk guard (ORC-114): room
//! enough, unless a debug build's [`FREE_SPACE_OVERRIDE`] says how much is free, as a QA copy of a low disk does.

use std::path::Path;

use orchester_core::workspaces::{DiskSpace, DiskSpaceError, DiskUsage, GIGABYTE};
use orchester_exec::disk_space::{FREE_SPACE_OVERRIDE, LocalDisks};

/// Free space while no override is set: far above what any project keeps free.
const ROOM_GB: u64 = 1 << 20;

pub(crate) struct Roomy;

impl DiskSpace for Roomy {
  fn usage(&self, folder: &Path) -> Result<DiskUsage, DiskSpaceError> {
    if std::env::var_os(FREE_SPACE_OVERRIDE).is_some() {
      return LocalDisks.usage(folder);
    }
    Ok(DiskUsage {
      free: ROOM_GB * GIGABYTE,
      total: 2 * ROOM_GB * GIGABYTE,
    })
  }
}
