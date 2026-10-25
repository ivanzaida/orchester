//! Bringing a finished run's work into the user's checkout and clearing what it left
//! (`docs/architecture/storage.md`, "Integration"): merging the run's branch into the base branch it
//! recorded when it started ([`merge_run`]; [`base`] says which branch that is and where the merge lands), and
//! removing the run's worktrees and the branches whose commits are merged ([`clean_up`]); [`Branches`] tells what
//! became of a succeeded run's work that Orchester did not merge. Everything runs through the `git` program with a
//! [`CommandRunner`](crate::command::CommandRunner); none of it stashes, resets or discards uncommitted work a
//! person did not confirm discarding.

pub mod base;
mod cleanup;
pub mod delivery;
mod drift;
mod forgotten;
mod fork;
mod git;
pub mod local;
mod log;
mod merge;
mod merged;
mod recorded;
pub mod resolve;

use std::path::Path;

use thiserror::Error;

pub use self::{
  cleanup::{
    Cleanup, CleanupError, KeepReason, LeftBranch, LeftWorktree, Leftovers, ListedWorktree, WORKTREES_DIR,
    WorktreeOwner, clean_up, clean_up_all, leftovers, leftovers_of, parse_worktree_list,
  },
  drift::{Drift, MAX_LANDED, branch_head, commit_subject, drift},
  forgotten::{FOLDER_ENTRIES_READ, FolderListing},
  fork::{Ancestry, RunFork, fork_point, made_on_branch, own_base, parse_log},
  log::{LoggedCommit, MAX_LISTED, own_commits},
  merge::{
    DeliveredAs, Delivering, MergeError, MergeKind, MergeOutcome, RunMerge, TemporaryLeft, merge_run, merge_run_as,
    merge_run_at,
  },
  merged::{Branches, Delivered, RunWork},
  recorded::{RecordedCleanupError, RemovalStart, clean_up_recorded, clean_up_recorded_as_seen},
};
use crate::{
  command::{CommandError, CommandRunner},
  run::BranchName,
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum GitError {
  #[error("cannot run git: {0}")]
  Run(#[from] CommandError),
  /// Git refused; its message, without credentials.
  #[error("git failed: {0}")]
  Failed(String),
  #[error("git printed something that is not {expected}: {detail}")]
  Unrecognized { expected: &'static str, detail: String },
}

/// The branch checked out in the repository at `repo`, read with the `git` program; `None` on a
/// detached HEAD. A run records it when it starts, as the branch merging it brings its work into.
pub fn checked_out_branch(
  runner: &impl CommandRunner,
  git: &Path,
  repo: &Path,
) -> Result<Option<BranchName>, GitError> {
  git::Git::new(runner, git, repo).checked_out_branch()
}
