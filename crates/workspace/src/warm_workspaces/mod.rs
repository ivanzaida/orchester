//! Warm workspaces as Project settings › Runs reads them (ORC-114): each workspace with what it does and its size,
//! the disk the workspaces are on, the warm folders with what the person's checkout holds of each, the suggestions
//! not taken yet, the warm-up and the worktrees runs made before workspaces. Reading walks folders to size them, so
//! it blocks: the UI calls it off the UI thread. What changes them is in [`actions`].

pub mod actions;
mod scan;

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  id::RunId,
  run::{BranchName, RunStatus},
  store::{RunStore, SettingsStore, TaskStore, WorkspaceStore},
  time::Timestamp,
  workspaces::{
    DiskSpace, DiskUsage, DiskVerdict, KeepFree, Suggestion, WarmFolder, WarmUpCommand, WarmUpFailure, WorkspaceRecord,
    WorkspaceState, suggest,
  },
};
use orchester_exec::disk_space::{LocalDisks, folder_size};
use orchester_store::{ProjectDb, StoreError};

pub use self::scan::ignored_folders;
use crate::tasks::{changes::run_worktree, detail::run_store_error};

/// One workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceRow {
  pub record: WorkspaceRecord,
  pub state: WorkspaceState,
  /// What its folder holds on disk; `None` when it could not be read.
  pub bytes: Option<u64>,
  /// The task key and run number of the run that holds it or used it last, for its row's words.
  pub run: Option<RunLabel>,
}

/// Which run a row names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunLabel {
  pub run: RunId,
  pub task_key: String,
  pub number: u32,
  /// The step it is at, while it holds the workspace.
  pub step: Option<String>,
}

/// A warm folder of the project and what the checkout holds of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderRow {
  pub folder: WarmFolder,
  /// How many folders of the checkout it names and their size; `None` when the checkout has none any more.
  pub found: Option<(u32, u64)>,
  /// It is among the suggestions: over the size worth keeping.
  pub suggested: bool,
}

/// A worktree a run made before workspaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OldWorktree {
  pub run: RunId,
  pub task_key: String,
  pub number: u32,
  pub bytes: Option<u64>,
  /// Its run still works: it goes when the run ends.
  pub in_use: bool,
  /// When its run ended, for an ended one.
  pub ended_at: Option<Timestamp>,
  pub branch: BranchName,
}

/// What Project settings › Runs shows of the workspaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspacesView {
  pub limit: NonZeroU32,
  pub rows: Vec<WorkspaceRow>,
  pub disk: Option<DiskUsage>,
  /// Whether the project turned warm workspaces on.
  pub enabled: bool,
  /// The free space the disk guard keeps, workspaces on or off.
  pub keep_free: KeepFree,
  pub folders: Vec<FolderRow>,
  /// Suggested folders the project has no warm folder for yet.
  pub suggestions: Vec<Suggestion>,
  pub warm_up: Option<WarmUpCommand>,
  pub warm_up_failure: Option<WarmUpFailure>,
  pub old_worktrees: Vec<OldWorktree>,
  /// The runs waiting for a free workspace, with since when.
  pub waits: Vec<(RunId, Timestamp)>,
}

impl WorkspacesView {
  /// Whether the disk guard holds work back: the disk has less free space than the project keeps.
  pub fn low_disk(&self) -> bool {
    self
      .disk
      .is_some_and(|usage| matches!(self.keep_free.verdict(usage), DiskVerdict::Low { .. }))
  }

  /// What the workspaces hold on disk together.
  pub fn workspace_bytes(&self) -> u64 {
    self.rows.iter().filter_map(|row| row.bytes).sum()
  }
}

/// The view of the project at `repo_root`, its disk read by `disks`.
pub fn read(repo_root: &Path, disks: &dyn DiskSpace) -> Result<WorkspacesView, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  let limit = db.project_limits()?.agent_slots().get();
  let settings = db.workspace_settings()?;
  let rows = db
    .workspaces()?
    .into_iter()
    .map(|record| row(&db, repo_root, record, limit))
    .collect::<Result<Vec<_>, _>>()?;
  let ignored = ignored_folders(repo_root);
  let suggested = suggest(&ignored);
  let folders = settings
    .folders
    .iter()
    .map(|folder| folder_row(folder, &ignored, &suggested))
    .collect();
  let suggestions = suggested
    .into_iter()
    .filter(|suggestion| settings.folder(&suggestion.pattern).is_none())
    .collect();
  Ok(WorkspacesView {
    limit,
    rows,
    disk: disks
      .usage(repo_root)
      .inspect_err(|error| tracing::warn!(%error, "the disk guard cannot read the free space"))
      .ok(),
    enabled: settings.enabled,
    keep_free: db.keep_free()?,
    folders,
    suggestions,
    warm_up: settings.warm_up,
    warm_up_failure: db.warm_up_failure()?,
    old_worktrees: old_worktrees(&db, repo_root)?,
    waits: db.workspace_waits()?,
  })
}

/// [`read`] of this computer's disk.
pub fn read_local(repo_root: &Path) -> Result<WorkspacesView, StoreError> {
  read(repo_root, &LocalDisks)
}

fn row(
  db: &ProjectDb,
  repo_root: &Path,
  record: WorkspaceRecord,
  limit: NonZeroU32,
) -> Result<WorkspaceRow, StoreError> {
  let run = record
    .holder
    .and_then(|holder| holder.run())
    .or(record.last_run)
    .map(|run| run_label(db, run, record.holder.is_some()))
    .transpose()?
    .flatten();
  Ok(WorkspaceRow {
    state: record.state(limit),
    bytes: folder_size(&record.number.path(repo_root)).ok(),
    run,
    record,
  })
}

fn run_label(db: &ProjectDb, run: RunId, holds: bool) -> Result<Option<RunLabel>, StoreError> {
  let Some(stored) = db.run(run).map_err(run_store_error)? else {
    return Ok(None);
  };
  let task_key = db
    .task(stored.task())?
    .map_or_else(String::new, |task| task.key().to_string());
  let step = holds
    .then(|| stored.running_executions().first().copied())
    .flatten()
    .and_then(|execution| stored.execution(execution))
    .and_then(|execution| stored.snapshot().body().step(execution.step()))
    .map(|step| step.name.as_str().to_owned());
  Ok(Some(RunLabel {
    run,
    task_key,
    number: stored.number().get(),
    step,
  }))
}

fn folder_row(
  folder: &WarmFolder,
  ignored: &[orchester_core::workspaces::IgnoredFolder],
  suggested: &[Suggestion],
) -> FolderRow {
  let named: Vec<u64> = ignored
    .iter()
    .filter(|found| folder.pattern.matches(&found.path))
    .map(|found| found.bytes)
    .collect();
  FolderRow {
    folder: folder.clone(),
    found: (!named.is_empty()).then(|| (u32::try_from(named.len()).unwrap_or(u32::MAX), named.iter().sum())),
    suggested: suggested.iter().any(|suggestion| suggestion.pattern == folder.pattern),
  }
}

/// The worktrees of runs made before workspaces, or while they are off, that are still there.
pub(crate) fn old_worktrees(db: &ProjectDb, repo_root: &Path) -> Result<Vec<OldWorktree>, StoreError> {
  let folder = repo_root.join(orchester_core::integration::WORKTREES_DIR);
  let Ok(entries) = std::fs::read_dir(&folder) else {
    return Ok(Vec::new());
  };
  let mut old = Vec::new();
  for entry in entries.flatten() {
    let Some(run) = entry.file_name().to_str().and_then(|name| name.parse::<RunId>().ok()) else {
      continue;
    };
    let Some(stored) = db.run(run).map_err(run_store_error)? else {
      continue;
    };
    let task_key = db
      .task(stored.task())?
      .map_or_else(String::new, |task| task.key().to_string());
    old.push(OldWorktree {
      run,
      task_key,
      number: stored.number().get(),
      bytes: folder_size(&run_worktree(repo_root, run)).ok(),
      in_use: stored.status().has_live_work() && !matches!(stored.status(), RunStatus::Interrupted(_)),
      ended_at: if stored.status().is_terminal() {
        db.events(run)
          .map_err(run_store_error)?
          .last()
          .map(orchester_core::run::RunEvent::at)
      } else {
        None
      },
      branch: stored.branch().clone(),
    });
  }
  old.sort_by_key(|worktree| worktree.number);
  Ok(old)
}
