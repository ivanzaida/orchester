//! What a person changes of the warm workspaces (ORC-114): the warm folders, the warm-up command and the free space
//! the disk guard keeps; cleaning idle workspaces' warm folders; and a stranded workspace's leftovers kept in a branch
//! or discarded. Each blocks on SQLite and git: the UI calls it off the UI thread.

use std::{path::Path, time::Duration};

use orchester_core::{
  integration::workspace::{WorkspaceGitError, Workspaces},
  run::BranchName,
  store::{RunStore, WorkspaceStore},
  workspaces::{WorkspaceNumber, WorkspaceSettings, rescue_branch},
};
use orchester_exec::{ProcessRunner, disk_space::folder_size};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use crate::tasks::{changes::GIT, detail::run_store_error};

/// Deleting a large build folder can take a while; only a git that hangs is stopped.
const GIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Error)]
pub enum WorkspaceActionError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Git(#[from] WorkspaceGitError),
  /// A workspace in use is never cleaned or rescued.
  #[error("workspace {0} is in use")]
  InUse(WorkspaceNumber),
  #[error("workspace {0} holds no leftovers")]
  NotStranded(WorkspaceNumber),
  #[error("the run is not there")]
  RunMissing,
  /// The old worktree was not removed, as the existing rules say: its run works, it holds uncommitted changes a
  /// person did not see here, or git refused.
  #[error("the old worktree was not removed: {0:?}")]
  NotRemoved(crate::tasks::integration::IntegrationProblem),
}

impl WorkspaceActionError {
  /// What the person reads.
  pub fn message(&self) -> crate::messages::Message {
    match self {
      Self::Store(error) => crate::messages::store_error(error),
      Self::NotRemoved(_) => crate::messages::Message::new("warmWorkspaces.notRemoved"),
      _ => crate::messages::Message::new("warmWorkspaces.notChanged"),
    }
  }
}

/// Removes the worktree run `run` made before workspaces, by the rules every removal follows: never while the run
/// works, never discarding uncommitted changes (the row shows only the folder's size, so none was seen), and keeping
/// every branch whose commits are not merged.
pub fn remove_old_worktree(repo_root: &Path, run: orchester_core::id::RunId) -> Result<(), WorkspaceActionError> {
  let stored = ProjectDb::open(repo_root)?
    .run(run)
    .map_err(run_store_error)?
    .ok_or(WorkspaceActionError::RunMissing)?;
  let programs = orchester_exec::programs::Programs::processes();
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  let mut seen = orchester_core::integration::leftovers(&runner, Path::new(GIT), repo_root, &stored)
    .map_err(|error| WorkspaceActionError::NotRemoved(crate::tasks::integration::IntegrationProblem::Read(error)))?;
  for worktree in &mut seen.worktrees {
    worktree.uncommitted.clear();
  }
  crate::tasks::integration::remove_in(repo_root, &programs, &stored, &seen).map_err(WorkspaceActionError::NotRemoved)
}

/// Saves `settings` for the project at `repo_root`.
pub fn save_settings(repo_root: &Path, settings: &WorkspaceSettings) -> Result<(), WorkspaceActionError> {
  ProjectDb::open(repo_root)?.save_workspace_settings(settings)?;
  Ok(())
}

/// Deletes the warm folders of each idle workspace of `numbers` (one in use is skipped and logged); the bytes that
/// freed. Their next run builds from cold; branches, checkpoints and the runs' artifacts stay.
pub fn clean(repo_root: &Path, numbers: &[WorkspaceNumber]) -> Result<u64, WorkspaceActionError> {
  let mut db = ProjectDb::open(repo_root)?;
  let records = db.workspaces()?;
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  let workspaces = Workspaces::new(&runner, Path::new(GIT), repo_root);
  let mut freed = 0_u64;
  for number in numbers {
    let Some(record) = records.iter().find(|record| record.number == *number) else {
      continue;
    };
    if record.holder.is_some() {
      tracing::warn!(%number, "a workspace in use is not cleaned");
      continue;
    }
    let path = number.path(repo_root);
    let before = folder_size(&path).unwrap_or(0);
    workspaces.clean_warm(&path)?;
    db.set_workspace_cold(*number)?;
    freed = freed.saturating_add(before.saturating_sub(folder_size(&path).unwrap_or(0)));
  }
  Ok(freed)
}

/// What becomes of a stranded workspace's leftovers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leftovers {
  /// "Keep them in a branch": `orchester/rescue/<task>-run-<n>`, from the branch of the run that left them.
  KeepInBranch,
  /// "Discard": a person saw them and chose to lose them.
  Discard,
}

/// Keeps or discards the leftovers of stranded workspace `number`, which is free again afterwards; the branch they
/// were kept on, when kept.
pub fn resolve_leftovers(
  repo_root: &Path,
  number: WorkspaceNumber,
  leftovers: Leftovers,
) -> Result<Option<BranchName>, WorkspaceActionError> {
  let mut db = ProjectDb::open(repo_root)?;
  let record = db
    .workspaces()?
    .into_iter()
    .find(|record| record.number == number)
    .ok_or(WorkspaceActionError::NotStranded(number))?;
  if record.holder.is_some() {
    return Err(WorkspaceActionError::InUse(number));
  }
  let stranded = record.stranded.ok_or(WorkspaceActionError::NotStranded(number))?;
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  let workspaces = Workspaces::new(&runner, Path::new(GIT), repo_root);
  let path = number.path(repo_root);
  let kept = match leftovers {
    Leftovers::Discard => {
      workspaces.discard(&path)?;
      None
    }
    Leftovers::KeepInBranch => {
      let run = stranded
        .run
        .map(|run| db.run(run).map_err(run_store_error))
        .transpose()?
        .flatten();
      let branch = match &run {
        Some(run) => rescue_branch(run.branch()),
        None => rescue_branch(&BranchName::new(&format!("workspace-{number}")).expect("a valid branch name")),
      };
      workspaces.rescue(
        &path,
        &branch,
        &format!("Kept from workspace {number}: what a stopped run left"),
      )?;
      Some(branch)
    }
  };
  db.set_workspace_stranded(number, None)?;
  Ok(kept)
}
