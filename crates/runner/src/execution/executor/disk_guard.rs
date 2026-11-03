//! The disk guard's reading of free space (ORC-114, ORC-422), which a run's task looks at before its steps start and
//! while its agents work (`super::held`). The scheduler's side, which holds new work back and resumes the runs paused
//! for low disk space, is `crate::execution::coordinator::low_disk`'s.

use std::{path::Path, sync::Arc, time::Duration};

use orchester_core::workspaces::{DiskSpace, DiskSpaceError, DiskUsage};

/// How often the disk guard looks at the free space while agents work, and while runs wait paused for low disk space.
const CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// The disk guard's reading of free space, and how often it looks.
#[derive(Clone)]
pub(in crate::execution) struct DiskGuard {
  pub(in crate::execution) space: Arc<dyn DiskSpace>,
  pub(in crate::execution) every: Duration,
}

impl DiskGuard {
  /// The guard over this computer's disks.
  pub(in crate::execution) fn local() -> Self {
    Self {
      space: Arc::new(orchester_exec::disk_space::LocalDisks),
      every: CHECK_INTERVAL,
    }
  }

  pub(in crate::execution) fn usage(&self, folder: &Path) -> Result<DiskUsage, DiskSpaceError> {
    self.space.usage(folder)
  }
}
