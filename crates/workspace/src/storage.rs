//! The disk guard as Project settings › Runs › Storage and Orchester's MCP tools read it (ORC-422): the free space the
//! project keeps, the repository's disk, whether the guard holds work back — in every project, warm workspaces on or
//! off — and, while it does, what could be cleaned to make room, in the order to clean
//! ([`orchester_core::workspaces::cleanable_order`]). The runner holds and resumes the work itself.

use std::path::Path;

use orchester_core::{
  store::{SettingsStore, WorkspaceStore},
  workspaces::{Cleanable, CleanableKind, DiskSpace, DiskUsage, KeepFree, cleanable_order},
};
use orchester_exec::disk_space::{LocalDisks, folder_size};
use orchester_store::{ProjectDb, StoreError};

use crate::{tasks::changes::run_worktree, warm_workspaces::old_worktrees};

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
  let db = ProjectDb::open(repo_root)?;
  let guard = DiskGuard::read(&db, repo_root, disks)?;
  let cleanable = if guard.holds() {
    cleanable(&db, repo_root)?
  } else {
    Vec::new()
  };
  Ok(Storage { guard, cleanable })
}

/// [`read`] of this computer's disk.
pub fn read_local(repo_root: &Path) -> Result<Storage, StoreError> {
  read(repo_root, &LocalDisks)
}

/// The worktrees of ended runs and the idle warm workspaces, sized, in the order to clean.
fn cleanable(db: &ProjectDb, repo_root: &Path) -> Result<Vec<CleanableRow>, StoreError> {
  let worktrees: Vec<_> = old_worktrees(db, repo_root)?
    .into_iter()
    .filter(|worktree| worktree.ended_at.is_some())
    .collect();
  let workspaces = db
    .workspaces()?
    .into_iter()
    .filter(|record| record.holder.is_none() && record.stranded.is_none())
    .map(|record| {
      let path = record.number.path(repo_root);
      Cleanable {
        kind: CleanableKind::IdleWorkspace { number: record.number },
        bytes: folder_size(&path).ok(),
        path,
      }
    });
  let ordered = cleanable_order(
    worktrees
      .iter()
      .map(|worktree| Cleanable {
        kind: CleanableKind::EndedRunWorktree { run: worktree.run },
        path: run_worktree(repo_root, worktree.run),
        bytes: worktree.bytes,
      })
      .chain(workspaces)
      .collect(),
  );
  Ok(
    ordered
      .into_iter()
      .map(|cleanable| {
        let run = match cleanable.kind {
          CleanableKind::EndedRunWorktree { run } => worktrees
            .iter()
            .find(|worktree| worktree.run == run)
            .map(|worktree| (worktree.task_key.clone(), worktree.number)),
          CleanableKind::IdleWorkspace { .. } => None,
        };
        CleanableRow { cleanable, run }
      })
      .collect(),
  )
}
