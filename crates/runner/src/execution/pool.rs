//! Borrowing a warm workspace for a run's work and giving it back (ORC-114, `orchester_core::workspaces`). A borrow
//! takes the workspace the store chooses, makes its folder when it has none yet, and switches it to the branch the
//! work is on. One that holds another run's uncommitted changes is stranded — nobody takes it until a person keeps
//! them in a branch or discards them — and the next one is tried. Giving back commits what is left, detaches the
//! workspace's HEAD so its branch is free, and records it idle.

use std::{path::Path, path::PathBuf, time::Duration};

use orchester_core::{
  integration::workspace::{SwitchTo, WorkspaceGitError, Workspaces},
  store::{SettingsStore, WorkspaceStore},
  time::Timestamp,
  workspaces::{Borrow, Holder, Stranded, WaitReason, WorkspaceNumber},
};
use orchester_exec::ProcessRunner;
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

/// Git here checks out branches of large checkouts and deletes build output: only a git that hangs is stopped.
const GIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const GIT: &str = "git";

#[derive(Debug, Error)]
pub(super) enum PoolError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("workspace {number}: {source}")]
  Git {
    number: WorkspaceNumber,
    source: WorkspaceGitError,
  },
}

/// What a borrow got.
pub(super) enum Borrowed {
  /// The folder of the workspace taken, switched to the work's branch.
  Folder(PathBuf),
  /// Every workspace is in use: the work waits.
  Wait(WaitReason),
}

/// Takes a workspace for `holder` in the repository at `root` and switches it to `to`; a new workspace's folder is
/// made at `base` first.
pub(super) fn borrow(db: &mut ProjectDb, root: &Path, holder: Holder, (to, base): (SwitchTo<'_>, &str)) -> Result<Borrowed, PoolError> {
  let limit = db.project_limits()?.agent_slots().get();
  let folders = db.workspace_settings()?.folders;
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  let workspaces = Workspaces::new(&runner, Path::new(GIT), root);
  let branch = match to {
    SwitchTo::Branch(branch) => Some(branch),
    SwitchTo::Commit(_) => None,
  };
  loop {
    let before = db.workspaces()?;
    let number = match db.borrow_workspace(holder, branch, limit, Timestamp::now())? {
      Borrow::Take(number) => number,
      Borrow::Wait(reason) => return Ok(Borrowed::Wait(reason)),
    };
    let last = before.iter().find(|record| record.number == number);
    let last_run = last.and_then(|record| record.last_run);
    // The run that worked there last goes on there, on the same branch: what it left goes along.
    let carry = last_run.is_some() && last_run == holder.run() && last.and_then(|record| record.branch.as_ref()) == branch;
    let switched = workspaces
      .prepare(number, base)
      .and_then(|path| workspaces.switch(&path, to, (&folders, carry)).map(|()| path));
    match switched {
      Ok(path) => return Ok(Borrowed::Folder(path)),
      Err(WorkspaceGitError::Uncommitted(files)) => {
        tracing::warn!(%number, files = files.len(), "a workspace holds uncommitted changes; it waits for a person");
        let files = u32::try_from(files.len()).unwrap_or(u32::MAX);
        db.set_workspace_stranded(number, Some(Stranded { run: last_run, files }))?;
        db.return_workspace(holder, Timestamp::now())?;
      }
      Err(source) => {
        db.return_workspace(holder, Timestamp::now())?;
        return Err(PoolError::Git { number, source });
      }
    }
  }
}

/// Gives back the workspace `holder` holds in the repository at `root`, its changes committed with `message` first;
/// `None` when it holds none. A commit or a detach git refuses is logged: the workspace is given back all the same,
/// and what is left in it makes the next borrower strand it for a person.
pub(super) fn give_back(
  db: &mut ProjectDb,
  root: &Path,
  holder: Holder,
  message: &str,
) -> Result<Option<WorkspaceNumber>, StoreError> {
  let Some(record) = db.workspaces()?.into_iter().find(|record| record.holder == Some(holder)) else {
    return Ok(None);
  };
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  let workspaces = Workspaces::new(&runner, Path::new(GIT), root);
  let path = record.number.path(root);
  if path.is_dir() {
    let saved = workspaces.commit(&path, message).and_then(|_| workspaces.detach(&path));
    if let Err(error) = saved {
      tracing::warn!(number = %record.number, %error, "a workspace given back keeps what it holds");
    }
  }
  db.return_workspace(holder, Timestamp::now())
}
