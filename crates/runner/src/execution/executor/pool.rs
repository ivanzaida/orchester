//! Borrowing a warm workspace for a run's work and giving it back (ORC-114, `orchester_core::workspaces`). A borrow
//! takes the workspace the store chooses, makes its folder when it has none yet, and switches it to the branch the
//! work is on. One that holds another run's uncommitted changes is stranded — nobody takes it until a person keeps
//! them in a branch or discards them — and the next one is tried. Giving back commits what is left, detaches the
//! workspace's HEAD so its branch is free, and records it idle.

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  integration::workspace::{SwitchTo, WorkspaceGitError, Workspaces},
  store::{SettingsStore, WorkspaceStore},
  time::Timestamp,
  workspaces::{Borrow, Holder, Stranded, WaitReason, WorkspaceNumber},
};
use orchester_exec::ProcessRunner;
use orchester_store::{ProjectDb, StoreError, WarmWorkspaceRecord, WorkspaceWritten};
use thiserror::Error;

use super::{
  link::{LinkError, SettingsLink, settings::warm_folders},
  sink::Sink,
};
use crate::execution_root::ExecutionRoot;

/// Git here checks out branches of large checkouts and deletes build output: only a git that hangs is stopped.
const GIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const GIT: &str = "git";

#[derive(Debug, Error)]
pub(super) enum PoolError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Link(#[from] LinkError),
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

/// Takes a workspace for `holder` in the execution root `root` and switches it to `to`, keeping or clearing the warm
/// folders asked through `link`; a new workspace's folder is made at `base` first. The project's workspaces are read
/// from `db`; the borrow and what follows it go to `sink`.
pub(super) fn borrow(
  (db, link, sink): (&ProjectDb, &(impl SettingsLink + ?Sized), &mut dyn Sink),
  root: &ExecutionRoot,
  holder: Holder,
  (to, base): (SwitchTo<'_>, &str),
) -> Result<Borrowed, PoolError> {
  let limit = db.project_limits()?.agent_slots().get();
  let folders = warm_folders(link)?;
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  let workspaces = Workspaces::new(&runner, Path::new(GIT), root.repository());
  let branch = match to {
    SwitchTo::Branch(branch) => Some(branch),
    SwitchTo::Commit(_) => None,
  };
  loop {
    let before = db.workspaces()?;
    let number = match take(
      sink,
      WarmWorkspaceRecord::borrow(holder, branch, limit, Timestamp::now()),
    )? {
      Borrow::Take(number) => number,
      Borrow::Wait(reason) => return Ok(Borrowed::Wait(reason)),
    };
    let last = before.iter().find(|record| record.number == number);
    let last_run = last.and_then(|record| record.last_run);
    // The run that worked there last goes on there, on the same branch: what it left goes along.
    let carry =
      last_run.is_some() && last_run == holder.run() && last.and_then(|record| record.branch.as_ref()) == branch;
    let switched = workspaces
      .prepare(number, base)
      .and_then(|path| workspaces.switch(&path, to, (&folders, carry)).map(|()| path));
    match switched {
      Ok(path) => return Ok(Borrowed::Folder(path)),
      Err(WorkspaceGitError::Uncommitted(files)) => {
        tracing::warn!(%number, files = files.len(), "a workspace holds uncommitted changes; it waits for a person");
        let files = u32::try_from(files.len()).unwrap_or(u32::MAX);
        sink.workspace(WarmWorkspaceRecord::stranded(
          number,
          Some(Stranded { run: last_run, files }),
        ))?;
        returned(sink, holder)?;
      }
      Err(source) => {
        returned(sink, holder)?;
        return Err(PoolError::Git { number, source });
      }
    }
  }
}

/// Gives back the workspace `holder` holds in the execution root `root`, its changes committed with `message` first;
/// `None` when it holds none. A commit or a detach git refuses is logged: the workspace is given back all the same,
/// and what is left in it makes the next borrower strand it for a person. The workspaces are read from `db`; the
/// return goes to `sink`.
pub(super) fn give_back(
  (db, sink): (&ProjectDb, &mut dyn Sink),
  root: &ExecutionRoot,
  holder: Holder,
  message: &str,
) -> Result<Option<WorkspaceNumber>, StoreError> {
  let Some(record) = db
    .workspaces()?
    .into_iter()
    .find(|record| record.holder == Some(holder))
  else {
    return Ok(None);
  };
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  let workspaces = Workspaces::new(&runner, Path::new(GIT), root.repository());
  let path = root.workspace(record.number);
  if path.is_dir() {
    let saved = workspaces.commit(&path, message).and_then(|_| workspaces.detach(&path));
    if let Err(error) = saved {
      tracing::warn!(number = %record.number, %error, "a workspace given back keeps what it holds");
    }
  }
  returned(sink, holder)
}

/// What the borrow `record` sent to `sink` gets.
fn take(sink: &mut dyn Sink, record: WarmWorkspaceRecord) -> Result<Borrow, StoreError> {
  match sink.workspace(record)? {
    WorkspaceWritten::Borrowed(borrow) => Ok(borrow),
    other => Err(unexpected(other)),
  }
}

/// Gives back the workspace `holder` holds, through `sink`: which it was, if it held one.
fn returned(sink: &mut dyn Sink, holder: Holder) -> Result<Option<WorkspaceNumber>, StoreError> {
  match sink.workspace(WarmWorkspaceRecord::give_back(holder, Timestamp::now()))? {
    WorkspaceWritten::Returned(number) => Ok(number),
    other => Err(unexpected(other)),
  }
}

/// A sink's answer of another kind than the record it was sent.
fn unexpected(answer: WorkspaceWritten) -> StoreError {
  StoreError::Corrupt {
    what: "workspace record answer",
    detail: format!("{answer:?}"),
  }
}
