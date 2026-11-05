//! The disk guard as Project settings › Runs › Storage and Orchester's MCP tools read it (ORC-422): the free space the
//! project keeps, the repository's disk, whether the guard holds work back — in every project, warm workspaces on or
//! off — and, while it does, what could be cleaned to make room, in the order to clean
//! ([`orchester_core::workspaces::cleanable_order`]). The runner holds and resumes the work itself, and sizes the
//! folders that could be cleaned (`docs/architecture/remote-runners.md`, rule 5).

mod inventory;
use std::path::Path;

pub use inventory::{KeptKind, KeptRow, inventory, largest_first, measure, measure_worktree, paused_runs};
use orchester_core::{
  integration::WORKTREES_DIR,
  store::{RunStore, SettingsStore, TaskStore, WorkspaceStore},
  workspaces::{Cleanable, CleanableKind, DiskSpace, DiskUsage, KeepFree, WorkspaceNumber, cleanable_order},
};
use orchester_exec::disk_space::LocalDisks;
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use crate::{
  runner::{ExecutorFolders, RunRequestError},
  tasks::{detail::run_store_error, run::control_message},
};

/// Why what the project's folders hold could not be read.
#[derive(Debug, Error)]
pub enum FoldersReadError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The runner, which lists and sizes the folders, did not answer.
  #[error(transparent)]
  Runner(#[from] RunRequestError),
}

impl FoldersReadError {
  /// What a person reads when the folders could not be read.
  pub fn message(&self) -> crate::messages::Message {
    match self {
      Self::Store(error) => crate::messages::store_error(error),
      Self::Runner(error) => control_message(error),
    }
  }
}

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

/// The storage of the project at `repo_root`, its disk read by `disks`. While the guard holds, the project's runner
/// (`folders`) walks the folders that could be cleaned to size them, so it blocks: callers read it off the UI thread
/// and off the async runtime.
pub fn read(repo_root: &Path, disks: &dyn DiskSpace, folders: &ExecutorFolders) -> Result<Storage, FoldersReadError> {
  let db = ProjectDb::open_read_only(repo_root)?;
  let guard = DiskGuard::read(&db, repo_root, disks)?;
  let cleanable = if guard.holds() {
    cleanable(&db, repo_root, folders)?
  } else {
    Vec::new()
  };
  Ok(Storage { guard, cleanable })
}

/// [`read`] of this computer's disk.
pub fn read_local(repo_root: &Path, folders: &ExecutorFolders) -> Result<Storage, FoldersReadError> {
  read(repo_root, &LocalDisks, folders)
}

/// The worktrees of ended runs, their own and their steps' (ORC-664), and the idle warm workspaces, sized by the
/// runner, in the order to clean.
fn cleanable(
  db: &ProjectDb,
  repo_root: &Path,
  folders: &ExecutorFolders,
) -> Result<Vec<CleanableRow>, FoldersReadError> {
  let idle: Vec<WorkspaceNumber> = db
    .workspaces()?
    .into_iter()
    .filter(|record| record.holder.is_none() && record.stranded.is_none())
    .map(|record| record.number)
    .collect();
  let sizes = folders.sizes(idle)?;
  let worktrees = ended_worktrees(db, repo_root, sizes.worktrees)?;
  let workspaces = sizes.workspaces.into_iter().map(|(number, bytes)| Cleanable {
    kind: CleanableKind::IdleWorkspace { number },
    path: number.path(repo_root),
    bytes,
  });
  let ordered = cleanable_order(
    worktrees
      .iter()
      .map(|row| row.cleanable.clone())
      .chain(workspaces)
      .collect(),
  );
  Ok(
    ordered
      .into_iter()
      .map(|cleanable| {
        let run = worktrees
          .iter()
          .find(|row| row.cleanable.path == cleanable.path)
          .and_then(|row| row.run.clone());
        CleanableRow { cleanable, run }
      })
      .collect(),
  )
}

/// Of the worktree folders the runner `found`, those of runs that ended, each with its run's task key and number.
fn ended_worktrees(
  db: &ProjectDb,
  repo_root: &Path,
  found: Vec<(String, Option<u64>)>,
) -> Result<Vec<CleanableRow>, StoreError> {
  let owners = db.worktree_folder_runs()?;
  let mut ended = Vec::new();
  for (name, bytes) in found {
    let Some(&run) = owners.get(&name) else {
      continue;
    };
    let Some(stored) = db.run(run).map_err(run_store_error)? else {
      continue;
    };
    if !stored.status().is_terminal() {
      continue;
    }
    let task = db
      .task(stored.task())?
      .map(|task| (task.key().to_string(), stored.number().get()));
    let cleanable = Cleanable {
      kind: CleanableKind::EndedRunWorktree { run },
      path: repo_root.join(WORKTREES_DIR).join(&name),
      bytes,
    };
    ended.push(CleanableRow { cleanable, run: task });
  }
  Ok(ended)
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
