//! Warm workspaces in the tests (ORC-114): a disk for the disk guard whose free space a test sets (plenty unless the
//! test lowers it), and where a run works.

use std::{
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use orchester_core::{
  id::RunId,
  store::WorkspaceStore,
  workspaces::{DiskSpace, DiskSpaceError, DiskUsage, GIGABYTE, Holder},
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use super::{scheduler_on, vault::MemoryVault};

/// A scheduler whose project turned warm workspaces on, so its runs borrow them and the disk guard reads `disk`.
pub fn pooled_scheduler(root: &Path, disk: TestDisk) -> (Scheduler, mpsc::UnboundedReceiver<Event>) {
  let settings = orchester_core::workspaces::WorkspaceSettings {
    enabled: true,
    ..Default::default()
  };
  ProjectDb::open(root)
    .expect("open")
    .save_workspace_settings(&settings)
    .expect("workspaces on");
  scheduler_on(root, Arc::new(MemoryVault::default()), disk)
}

/// Where run `id` works: the workspace it holds, else the one it worked in last, else its own worktree from before
/// workspaces.
pub fn run_folder(root: &Path, id: RunId) -> PathBuf {
  let records = ProjectDb::open(root).expect("open").workspaces().expect("workspaces");
  records
    .iter()
    .find(|record| record.holder == Some(Holder::Run { run: id }))
    .or_else(|| {
      records
        .iter()
        .filter(|record| record.last_run == Some(id))
        .max_by_key(|record| record.last_used)
    })
    .map_or_else(
      || root.join(".orchester/worktrees").join(id.to_string()),
      |record| record.number.path(root),
    )
}

/// Gigabytes free while a test does not say otherwise: far above what a project keeps free.
const PLENTY_GB: u64 = 10_000;

#[derive(Clone)]
pub struct TestDisk(Arc<AtomicU64>);

impl Default for TestDisk {
  fn default() -> Self {
    Self(Arc::new(AtomicU64::new(PLENTY_GB)))
  }
}

impl TestDisk {
  /// The disk has `gigabytes` free from now on.
  pub fn set_free(&self, gigabytes: u64) {
    self.0.store(gigabytes, Ordering::Release);
  }
}

impl DiskSpace for TestDisk {
  fn usage(&self, _folder: &Path) -> Result<DiskUsage, DiskSpaceError> {
    let free = self.0.load(Ordering::Acquire) * GIGABYTE;
    Ok(DiskUsage {
      free,
      total: free.max(2 * PLENTY_GB * GIGABYTE),
    })
  }
}
