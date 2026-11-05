//! Storage's full inventory, listed by the project's runner without sizing first, then measured row by row off the
//! UI thread, each row by the runner too (`docs/architecture/remote-runners.md`, rule 5). Which rows there are and
//! what each says come from the project database here.
use std::{
  cmp::Reverse,
  path::{Path, PathBuf},
};

use orchester_core::{
  id::RunId,
  integration::WORKTREES_DIR,
  run::{PauseReason, RunStatus},
  store::{RunStore, TaskStore, WorkspaceStore},
  time::Timestamp,
  workspaces::{CleanableKind, Holder},
};
use orchester_ipc::folders::ExecutorFolder;
use orchester_store::{ProjectDb, StoreError};

use super::FoldersReadError;
use crate::{runner::ExecutorFolders, tasks::detail::run_store_error};

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

  /// The executor's folder the row is, as the runner names it.
  fn folder(&self) -> Option<ExecutorFolder> {
    match self.kind {
      KeptKind::Worktree(_) => worktree_folder(&self.path),
      KeptKind::Workspace(number) => Some(ExecutorFolder::Workspace { number }),
    }
  }
}

/// The worktrees folder's folder at `path`, by its name.
fn worktree_folder(path: &Path) -> Option<ExecutorFolder> {
  let name = path.file_name()?.to_str()?;
  Some(ExecutorFolder::Worktree { name: name.to_owned() })
}

/// The rows of the project at `repo_root`, none sized: the worktrees folder's folders a run of the project owns, its
/// own or a step's, and the warm workspaces whose folder is there, as its runner (`folders`) finds them. It blocks on
/// the runner: read it off the UI thread and off the async runtime.
pub fn inventory(repo_root: &Path, folders: &ExecutorFolders) -> Result<Vec<KeptRow>, FoldersReadError> {
  let db = ProjectDb::open_read_only(repo_root)?;
  let records = db.workspaces()?;
  let present = folders.list(records.iter().map(|record| record.number).collect())?;
  let mut rows = Vec::new();
  if !present.worktrees.is_empty() {
    let owners = db.worktree_folder_runs()?;
    for name in present.worktrees {
      let Some(&run) = owners.get(&name) else {
        continue;
      };
      if let Some(row) = run_row(&db, run, repo_root.join(WORKTREES_DIR).join(&name))? {
        rows.push(row);
      }
    }
  }
  for record in records {
    if !present.workspaces.contains(&record.number) {
      continue;
    }
    let path = record.number.path(repo_root);
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

/// One row measured by the project's runner (`folders`), which blocks; an unreadable size, or a runner that did not
/// answer, stays unknown instead of becoming zero.
pub fn measure(row: &mut KeptRow, folders: &ExecutorFolders) {
  row.bytes = row.folder().and_then(|folder| measured(folders, folder, &row.path));
}

/// The size of the worktree folder at `path`, one of the worktrees folder's, measured by the project's runner
/// (`folders`), which blocks; unknown as in [`measure`].
pub fn measure_worktree(folders: &ExecutorFolders, path: &Path) -> Option<u64> {
  worktree_folder(path).and_then(|folder| measured(folders, folder, path))
}

fn measured(folders: &ExecutorFolders, folder: ExecutorFolder, path: &Path) -> Option<u64> {
  folders
    .measure(folder)
    .inspect_err(|error| tracing::warn!(%error, path = %path.display(), "cannot measure storage folder"))
    .ok()
    .flatten()
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
