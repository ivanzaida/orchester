//! What the coordinator hands the executor for a run it recorded (`docs/architecture/remote-runners.md`, "The
//! split"): the coordinator reads the person's checkout for the run's base and names its branch; the executor makes
//! the run's place to work at that revision, on that branch or the first free numbered name of it, and answers with
//! the branch it made or why it could not ([`WorktreeFailure`](super::WorktreeFailure)).

use serde::{Deserialize, Serialize};

use super::BranchName;
use crate::{id::RunId, revision::Revision};

/// The order to make the place a recorded run starts in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartOrder {
  run: RunId,
  branch: BranchName,
  base: Revision,
  borrows: bool,
}

impl StartOrder {
  /// The order for `run` on `branch` at `base`; `borrows` when the project lends warm workspaces, so a run without a
  /// worktree of its own gets only its branch.
  pub const fn new(run: RunId, branch: BranchName, base: Revision, borrows: bool) -> Self {
    Self {
      run,
      branch,
      base,
      borrows,
    }
  }

  pub const fn run(&self) -> RunId {
    self.run
  }

  /// The branch the coordinator named; the executor takes the first free numbered name when it is taken.
  pub const fn branch(&self) -> &BranchName {
    &self.branch
  }

  /// The revision the run starts from.
  pub const fn base(&self) -> &Revision {
    &self.base
  }

  /// Whether the project lends warm workspaces to runs.
  pub const fn borrows(&self) -> bool {
    self.borrows
  }
}
