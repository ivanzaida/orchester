//! Removing what a run left before its task runs again (`docs/architecture/remote-runners.md`, "The split": the
//! executor removes run worktrees), with the same rules as *Remove worktree* ([`clean_up_recorded`]): a worktree with
//! uncommitted changes goes only with exactly the changes the person saw, and the removal is recorded first, through
//! the coordinator's sink ([`SinkWorktrees`]). The codebase indexes only removed worktrees used go with them, and the
//! worktree folders are swept after.

use std::{path::Path, time::Duration};

use orchester_core::{
  id::{RunId, TaskId},
  integration::{KeepReason, Leftovers, NotRemoved, RemovalStart, clean_up_recorded, finish_removal},
  run::Run,
  store::WorktreeStore,
  time::Timestamp,
};
use orchester_exec::{ProcessRunner, run_processes::RunProcesses};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, RemovalRecord, StoreError};

use super::{failed, sink::Sink, worktree_sweep};
use crate::execution_root::ExecutionRoot;

/// Git removing a worktree full of build output gets as long as the desktop gave it.
pub(super) const REMOVE_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// Removes what `previous` left in the repository at `root`, as the person confirmed it (`confirmed`), with the
/// removal recorded through `sink` first; a folder something held is deleted again once the run's own processes there
/// are stopped, and one that still stays is recorded as not removed ([`finish_removal`]). Refused when a worktree
/// stays. The removals recorded are read from `db`.
pub(in crate::execution) fn remove_worktrees(
  (db, sink): (&ProjectDb, &mut dyn Sink),
  root: &Path,
  previous: &Run,
  confirmed: &Leftovers,
) -> Result<(), Rejection> {
  let git = ProcessRunner::new(REMOVE_TIMEOUT);
  let git = (&git, Path::new("git"), root);
  let mut store = SinkWorktrees { db, sink };
  let mut cleanup = clean_up_recorded(&mut store, git, previous, Some(confirmed), Timestamp::now())
    .map_err(|error| failed("cannot remove the worktrees of the run before", &error))?;
  let stop = |folder: &Path| RunProcesses::stop_in(folder, previous.created_at());
  if let Err(error) = finish_removal(&mut store, git, (previous.id(), Timestamp::now()), &mut cleanup, stop) {
    tracing::warn!(%error, run = %previous.id(), "cannot record what the worktree removal left");
  }
  after_removal(root, true);
  match cleanup.kept_worktrees.into_iter().next() {
    None => Ok(()),
    Some((worktree, KeepReason::Failed(detail))) => {
      tracing::warn!(path = %worktree.path, %detail, "git did not remove the worktree of the run before");
      Err(Rejection::WorktreeNotRemoved {
        path: worktree.path,
        detail,
      })
    }
    Some((worktree, _)) => Err(Rejection::WorktreeChanged { path: worktree.path }),
  }
}

/// What follows a removal of run worktrees in the repository at `root`: the codebase indexes only removed worktrees
/// used go, off the runtime's async workers (the worktrees are gone either way, and an index left behind goes with
/// the next removal), and, when `sweep` says so, the worktree folders are swept ([`worktree_sweep`]).
pub(super) fn after_removal(root: &Path, sweep: bool) {
  let indexes = ExecutionRoot::local(root).code_indexes();
  tokio::task::spawn_blocking(move || {
    if let Err(error) = indexes.prune() {
      tracing::warn!(%error, "cannot remove the codebase indexes of removed worktrees");
    }
  });
  if sweep {
    worktree_sweep::sweep_in_background(root.to_path_buf());
  }
}

/// The worktree removals as the executor keeps them: read from the project database, written through the
/// coordinator's sink, each change as the store applies it.
struct SinkWorktrees<'a> {
  db: &'a ProjectDb,
  sink: &'a mut dyn Sink,
}

impl SinkWorktrees<'_> {
  /// Sends `record`, which begins no removal.
  fn send(&mut self, record: RemovalRecord) -> Result<(), StoreError> {
    self.sink.removal(record).map(drop)
  }
}

impl WorktreeStore for SinkWorktrees<'_> {
  type Error = StoreError;

  fn save_worktree_removal(&mut self, run: RunId, at: Timestamp) -> Result<(), StoreError> {
    self.send(RemovalRecord::found(run, at))
  }

  fn begin_worktree_removal(&mut self, run: RunId, at: Timestamp) -> Result<RemovalStart, StoreError> {
    self
      .sink
      .removal(RemovalRecord::begun(run, at))?
      .ok_or_else(|| StoreError::Corrupt {
        what: "worktree removal",
        detail: "beginning a removal found nothing".to_owned(),
      })
  }

  fn undo_worktree_removal(&mut self, run: RunId, at: Timestamp) -> Result<(), StoreError> {
    self.send(RemovalRecord::undone(run, at))
  }

  fn worktree_removal(&self, run: RunId) -> Result<Option<Timestamp>, StoreError> {
    self.db.worktree_removal(run)
  }

  fn worktree_removals(&self, task: TaskId) -> Result<Vec<(RunId, Timestamp)>, StoreError> {
    self.db.worktree_removals(task)
  }

  fn save_worktree_not_removed(&mut self, run: RunId, left: &NotRemoved, at: Timestamp) -> Result<(), StoreError> {
    self.send(RemovalRecord::not_removed(run, left, at))
  }

  fn clear_worktree_not_removed(&mut self, run: RunId) -> Result<(), StoreError> {
    self.send(RemovalRecord::all_removed(run))
  }
}
