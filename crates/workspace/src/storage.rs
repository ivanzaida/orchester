//! The disk guard as Project settings › Runs › Storage and Orchester's MCP tools read it (ORC-422): the free space the
//! project keeps, the repository's disk, whether the guard holds work back — in every project, warm workspaces on or
//! off — and, while it does, what could be cleaned to make room, in the order to clean
//! ([`orchester_core::workspaces::cleanable_order`]). The runner holds and resumes the work itself.

mod inventory;
use std::path::Path;

pub use inventory::{KeptKind, KeptRow, inventory, largest_first, measure, paused_runs};
use orchester_core::{
  store::SettingsStore,
  workspaces::{Cleanable, DiskSpace, DiskUsage, KeepFree, cleanable_order},
};
use orchester_exec::disk_space::LocalDisks;
use orchester_store::{ProjectDb, StoreError};

/// The disk guard of a project as it stands now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskGuard {
  pub keep_free: KeepFree,
  /// The repository's disk; `None` when it could not be read, which holds nothing back.
  pub disk: Option<DiskUsage>,
}

impl DiskGuard {
  /// The guard of the project at `repo_root`, its disk read by `disks`.
  pub fn read(db: &ProjectDb, repo_root: &Path, disks: &dyn DiskSpace) -> Result<Self, StoreError> {
    Ok(Self {
      keep_free: db.keep_free()?,
      disk: disks
        .usage(repo_root)
        .inspect_err(|error| tracing::warn!(%error, "the disk guard cannot read the free space"))
        .ok(),
    })
  }

  /// Whether the guard holds work back: no step starts and working steps pause.
  pub fn holds(&self) -> bool {
    self.disk.is_some_and(|usage| self.keep_free.verdict(usage).is_low())
  }
}

/// The disk guard, and what could be cleaned while it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Storage {
  pub guard: DiskGuard,
  /// In the order to clean; empty while the guard holds nothing back.
  pub cleanable: Vec<CleanableRow>,
}

/// A folder that could be cleaned, with the run whose worktree it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanableRow {
  pub cleanable: Cleanable,
  /// The task key and run number of an ended run's worktree.
  pub run: Option<(String, u32)>,
}

/// The storage of the project at `repo_root`, its disk read by `disks`. While the guard holds it walks the folders
/// that could be cleaned to size them, so it blocks: callers read it off the UI thread.
pub fn read(repo_root: &Path, disks: &dyn DiskSpace) -> Result<Storage, StoreError> {
  let db = ProjectDb::open_read_only(repo_root)?;
  let guard = DiskGuard::read(&db, repo_root, disks)?;
  let cleanable = if guard.holds() {
    cleanable(repo_root)?
  } else {
    Vec::new()
  };
  Ok(Storage { guard, cleanable })
}

/// [`read`] of this computer's disk.
pub fn read_local(repo_root: &Path) -> Result<Storage, StoreError> {
  read(repo_root, &LocalDisks)
}

/// The worktrees of ended runs and idle warm workspaces, with the inventory's same rules.
fn cleanable(repo_root: &Path) -> Result<Vec<CleanableRow>, StoreError> {
  let mut rows = inventory(repo_root)?;
  rows.retain(|row| row.cleanable.is_some());
  for row in &mut rows {
    measure(row);
  }
  let ordered = cleanable_order(
    rows
      .iter()
      .filter_map(|row| {
        row.cleanable.map(|kind| Cleanable {
          kind,
          path: row.path.clone(),
          bytes: row.bytes,
        })
      })
      .collect(),
  );
  Ok(
    ordered
      .into_iter()
      .map(|cleanable| {
        let run = rows
          .iter()
          .find(|row| row.path == cleanable.path)
          .and_then(|row| row.task.as_ref().map(|(key, _, number)| (key.clone(), *number)));
        CleanableRow { cleanable, run }
      })
      .collect(),
  )
}

/// Saving a validated KeepFree is the same operation for the window and MCP.
pub fn save_keep_free(repo_root: &Path, keep: KeepFree) -> Result<(), StoreError> {
  ProjectDb::open(repo_root)?.save_keep_free(keep)
}

/// The shared activity message while Storage removes confirmed worktrees.
pub fn cleanup_running() -> crate::messages::Message {
  crate::messages::Message::new("settings.project.runs.storage.clean_up.running")
}

/// Summarizes only worktrees actually removed; unreadable sizes stay explicitly unknown.
pub fn cleanup_outcome(failed: usize, removed: usize, freed: Option<u64>) -> crate::messages::Message {
  let key = match (failed == 0, freed.is_some()) {
    (true, true) => "settings.project.runs.storage.clean_up.done",
    (false, true) => "settings.project.runs.storage.clean_up.partial",
    (true, false) => "settings.project.runs.storage.clean_up.doneUnknown",
    (false, false) => "settings.project.runs.storage.clean_up.partialUnknown",
  };
  crate::messages::Message::new(key)
    .with("count", if failed == 0 { removed } else { failed }.to_string())
    .with(
      "size",
      freed.map_or_else(String::new, |n| {
        format!("{:.1}", n as f64 / orchester_core::workspaces::GIGABYTE as f64)
      }),
    )
}

/// A cheap, read-only guard read, suitable for the status bar; it never sizes folders.
pub fn guard_local(repo_root: &Path) -> Result<DiskGuard, StoreError> {
  let db = ProjectDb::open_read_only(repo_root)?;
  DiskGuard::read(&db, repo_root, &LocalDisks)
}
