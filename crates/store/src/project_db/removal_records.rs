//! What the runner records when it removes a run's worktrees (ORC-63, ORC-418), as values that can be sent
//! (`docs/architecture/remote-runners.md`, rule 2): the removal found or begun — which also drops a merge waiting in
//! the worktree (`run_resolutions`) — taken back, and the folder it left or that it left none. Each record holds the
//! arguments of its [`WorktreeStore`] function, and [`ProjectDb::write_removal`] applies it with that function, in its
//! transaction: beginning still reads the run's status and records the removal in one immediate transaction.

use orchester_core::{
  id::RunId,
  integration::{NotRemoved, RemovalStart},
  store::WorktreeStore,
  time::Timestamp,
};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::StoreError;

/// A change to the record of a run's worktree removal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovalRecord(RemovalChange);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum RemovalChange {
  Found {
    run: RunId,
    at: Timestamp,
  },
  Begun {
    run: RunId,
    at: Timestamp,
  },
  Undone {
    run: RunId,
    at: Timestamp,
  },
  NotRemoved {
    run: RunId,
    path: String,
    reason: Option<String>,
    at: Timestamp,
  },
  AllRemoved {
    run: RunId,
  },
}

impl RemovalRecord {
  /// The worktree of `run` was found removed at `at` ([`WorktreeStore::save_worktree_removal`]).
  pub const fn found(run: RunId, at: Timestamp) -> Self {
    Self(RemovalChange::Found { run, at })
  }

  /// The worktrees of `run` are removed at `at`, unless it has live work
  /// ([`WorktreeStore::begin_worktree_removal`]).
  pub const fn begun(run: RunId, at: Timestamp) -> Self {
    Self(RemovalChange::Begun { run, at })
  }

  /// The removal begun at `at` is taken back ([`WorktreeStore::undo_worktree_removal`]).
  pub const fn undone(run: RunId, at: Timestamp) -> Self {
    Self(RemovalChange::Undone { run, at })
  }

  /// Removing the worktrees of `run` left `left` at `at` ([`WorktreeStore::save_worktree_not_removed`]).
  pub fn not_removed(run: RunId, left: &NotRemoved, at: Timestamp) -> Self {
    Self(RemovalChange::NotRemoved {
      run,
      path: left.path.clone(),
      reason: left.reason.clone(),
      at,
    })
  }

  /// Removing the worktrees of `run` left nothing ([`WorktreeStore::clear_worktree_not_removed`]).
  pub const fn all_removed(run: RunId) -> Self {
    Self(RemovalChange::AllRemoved { run })
  }
}

impl ProjectDb {
  /// Applies `record` with its store function: what beginning a removal found, for a record that begins one; `None`
  /// for any other.
  pub fn write_removal(&mut self, record: &RemovalRecord) -> Result<Option<RemovalStart>, StoreError> {
    match &record.0 {
      RemovalChange::Found { run, at } => self.save_worktree_removal(*run, *at).map(|()| None),
      RemovalChange::Begun { run, at } => self.begin_worktree_removal(*run, *at).map(Some),
      RemovalChange::Undone { run, at } => self.undo_worktree_removal(*run, *at).map(|()| None),
      RemovalChange::NotRemoved { run, path, reason, at } => {
        let left = NotRemoved {
          path: path.clone(),
          reason: reason.clone(),
        };
        self.save_worktree_not_removed(*run, &left, *at).map(|()| None)
      }
      RemovalChange::AllRemoved { run } => self.clear_worktree_not_removed(*run).map(|()| None),
    }
  }
}
