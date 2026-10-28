//! Free space on macOS and other Unix systems: `statvfs`, the blocks available to this process.

use std::{io, path::Path};

use orchester_core::workspaces::DiskUsage;

pub(super) fn usage(folder: &Path) -> io::Result<DiskUsage> {
  let stat = rustix::fs::statvfs(folder).map_err(io::Error::from)?;
  Ok(DiskUsage {
    free: stat.f_bavail.saturating_mul(stat.f_frsize),
    total: stat.f_blocks.saturating_mul(stat.f_frsize),
  })
}
