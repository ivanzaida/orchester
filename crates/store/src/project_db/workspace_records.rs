//! What the runner records when a run borrows a warm workspace or gives it back (ORC-114), as values that can be sent
//! (`docs/architecture/remote-runners.md`, rule 2): a borrow, with the wait it records when every workspace is in use,
//! a return, a workspace stranded with a run's uncommitted changes, and a run's wait that ended. Each record holds the
//! arguments of its store function, and [`ProjectDb::write_workspace`] applies it with that function, in its
//! transaction: a borrow still reads the workspaces and records its choice in one immediate transaction.

use std::num::NonZeroU32;

use orchester_core::{
  id::RunId,
  run::BranchName,
  store::WorkspaceStore,
  time::Timestamp,
  workspaces::{Borrow, Holder, Stranded, WorkspaceNumber},
};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::StoreError;

/// A change to the project's warm workspaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmWorkspaceRecord(WorkspaceChange);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum WorkspaceChange {
  Borrow {
    holder: Holder,
    branch: Option<BranchName>,
    limit: NonZeroU32,
    at: Timestamp,
  },
  Return {
    holder: Holder,
    at: Timestamp,
  },
  Stranded {
    number: WorkspaceNumber,
    stranded: Option<Stranded>,
  },
  WaitEnded {
    run: RunId,
  },
}

/// What a workspace record did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceWritten {
  /// A borrow: what the borrower gets.
  Borrowed(Borrow),
  /// A return: the workspace given back; `None` when the holder held none.
  Returned(Option<WorkspaceNumber>),
  /// Any other change, made.
  Changed,
}

impl WarmWorkspaceRecord {
  /// `holder` takes a workspace for `branch` among `limit` at `at`, or waits
  /// ([`WorkspaceStore::borrow_workspace`]).
  pub fn borrow(holder: Holder, branch: Option<&BranchName>, limit: NonZeroU32, at: Timestamp) -> Self {
    Self(WorkspaceChange::Borrow {
      holder,
      branch: branch.cloned(),
      limit,
      at,
    })
  }

  /// `holder` gives back the workspace it holds at `at` ([`WorkspaceStore::return_workspace`]).
  pub const fn give_back(holder: Holder, at: Timestamp) -> Self {
    Self(WorkspaceChange::Return { holder, at })
  }

  /// Workspace `number` holds a run's uncommitted changes, or no longer does
  /// ([`WorkspaceStore::set_workspace_stranded`]).
  pub const fn stranded(number: WorkspaceNumber, stranded: Option<Stranded>) -> Self {
    Self(WorkspaceChange::Stranded { number, stranded })
  }

  /// `run` waits for a workspace no more ([`WorkspaceStore::end_workspace_wait`]).
  pub const fn wait_ended(run: RunId) -> Self {
    Self(WorkspaceChange::WaitEnded { run })
  }
}

impl ProjectDb {
  /// Applies `record` with its store function.
  pub fn write_workspace(&mut self, record: &WarmWorkspaceRecord) -> Result<WorkspaceWritten, StoreError> {
    Ok(match &record.0 {
      WorkspaceChange::Borrow {
        holder,
        branch,
        limit,
        at,
      } => WorkspaceWritten::Borrowed(self.borrow_workspace(*holder, branch.as_ref(), *limit, *at)?),
      WorkspaceChange::Return { holder, at } => WorkspaceWritten::Returned(self.return_workspace(*holder, *at)?),
      WorkspaceChange::Stranded { number, stranded } => {
        self.set_workspace_stranded(*number, *stranded)?;
        WorkspaceWritten::Changed
      }
      WorkspaceChange::WaitEnded { run } => {
        self.end_workspace_wait(*run)?;
        WorkspaceWritten::Changed
      }
    })
  }
}
