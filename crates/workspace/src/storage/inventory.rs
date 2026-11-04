//! Storage's full inventory, read without sizing first, then measured off the UI thread.
use std::{
  cmp::Reverse,
  path::{Path, PathBuf},
};

use orchester_core::{
  id::RunId,
  run::{PauseReason, RunStatus},
  store::{RunStore, TaskStore, WorkspaceStore},
  time::Timestamp,
  workspaces::{CleanableKind, Holder},
};
use orchester_exec::disk_space::folder_size;
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::detail::run_store_error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeptKind {
  Worktree(RunId),
  Workspace(orchester_core::workspaces::WorkspaceNumber),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptRow {
  pub kind: KeptKind,
  pub path: PathBuf,
  pub run: Option<RunId>,
  pub task: Option<(String, String, u32)>,
  pub status: Option<RunStatus>,
  pub last_used: Option<Timestamp>,
  pub bytes: Option<u64>,
  pub in_use: bool,
  pub removal: Option<orchester_core::integration::NotRemoved>,
  /// The cleanup rules the MCP's cleanable list also uses.
  pub cleanable: Option<CleanableKind>,
}

impl KeptRow {
  pub fn removable_run(&self) -> Option<RunId> {
    match self.cleanable {
      Some(CleanableKind::EndedRunWorktree { run }) => Some(run),
      _ => None,
    }
  }
}

pub fn inventory(repo_root: &Path) -> Result<Vec<KeptRow>, StoreError> {
  let db = ProjectDb::open_read_only(repo_root)?;
  let mut rows = Vec::new();
  let folder = repo_root.join(orchester_core::integration::WORKTREES_DIR);
  // A new project has no worktree directory; other IO failures must not look empty.
  let entries = match std::fs::read_dir(&folder) {
    Ok(entries) => Some(entries),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
    Err(source) => return Err(StoreError::Io { path: folder, source }),
  };
  if let Some(entries) = entries {
    let owners = db.worktree_folder_runs()?;
    for entry in entries {
      let entry = entry.map_err(|source| StoreError::Io {
        path: folder.clone(),
        source,
      })?;
      let name = entry.file_name();
      let Some(run) = name.to_str().and_then(|name| owners.get(name)).copied() else {
        continue;
      };
      let path = entry.path();
      if path.is_dir()
        && let Some(row) = run_row(&db, run, path)?
      {
        rows.push(row);
      }
    }
  }
  for record in db.workspaces()? {
    let path = record.number.path(repo_root);
    if !path.is_dir() {
      continue;
    }
    let held = match record.holder {
      Some(Holder::Run { run }) => Some(run),
      _ => None,
    };
    let run = held.or(record.last_run);
    let label = run.map(|run| run_row(&db, run, path.clone())).transpose()?.flatten();
    rows.push(KeptRow {
      kind: KeptKind::Workspace(record.number),
      path,
      run,
      task: label.as_ref().and_then(|r| r.task.clone()),
      status: label.and_then(|r| r.status),
      last_used: record.last_used,
      bytes: None,
      removal: None,
      in_use: record.holder.is_some() || record.stranded.is_some(),
      cleanable: (record.holder.is_none() && record.stranded.is_none())
        .then_some(CleanableKind::IdleWorkspace { number: record.number }),
    });
  }
  Ok(rows)
}

fn run_row(db: &ProjectDb, run: RunId, path: PathBuf) -> Result<Option<KeptRow>, StoreError> {
  let Some(stored) = db.run(run).map_err(run_store_error)? else {
    return Ok(None);
  };
  let task = db.task(stored.task())?.map(|task| {
    (
      task.key().to_string(),
      task.details().title.to_string(),
      stored.number().get(),
    )
  });
  let last_used = db
    .events(run)
    .map_err(run_store_error)?
    .last()
    .map(orchester_core::run::RunEvent::at);
  let removal = db
    .worktree_not_removed(run)?
    .filter(|left| Path::new(&left.path) == path);
  Ok(Some(KeptRow {
    kind: KeptKind::Worktree(run),
    path,
    run: Some(run),
    task,
    status: Some(stored.status().clone()),
    last_used,
    bytes: None,
    removal,
    in_use: stored.status().has_live_work(),
    cleanable: stored
      .status()
      .is_terminal()
      .then_some(CleanableKind::EndedRunWorktree { run }),
  }))
}

/// One measured row; an unreadable size stays unknown instead of becoming zero.
pub fn measure(row: &mut KeptRow) {
  row.bytes = folder_size(&row.path)
    .inspect_err(|error| tracing::warn!(%error, path = %row.path.display(), "cannot measure storage folder"))
    .ok();
}

pub fn largest_first(rows: &mut [KeptRow]) {
  rows.sort_by_key(|row| Reverse(row.bytes));
}

/// Counts only runs actually paused by the disk guard, never all paused runs.
pub fn paused_runs(repo_root: &Path) -> Result<usize, StoreError> {
  let db = ProjectDb::open_read_only(repo_root)?;
  let runs = db.low_disk_runs().map_err(run_store_error)?;
  let runs = runs
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|bad| run_store_error(bad.error))?;
  Ok(
    runs
      .iter()
      .filter(|run| matches!(run.status(), RunStatus::Paused(PauseReason::LowDisk)))
      .count(),
  )
}
