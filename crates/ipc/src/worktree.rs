//! What a run's worktree holds beyond its checked-out commit, read by the runner where the worktree is
//! ([`crate::Command::ReadWorktreeChanges`], protocol 40): the desktop reads no run's worktree itself
//! (`docs/architecture/remote-runners.md`, rule 5). The diff travels as git printed it, cut as Orchester cuts every
//! diff it reads; the desktop parses it.

use orchester_core::{diff::Changes, revision::Revision};
use serde::{Deserialize, Serialize};

/// What the runner read in a run's worktree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorktreeChangesRead {
  /// The run has no worktree folder: not made yet, removed, or missing; the desktop tells which by the run's stage.
  NoWorktree,
  /// Nothing beyond its checked-out commit.
  Clean,
  /// The unified diff from `head`, the commit the worktree has checked out, to its files; untracked files are added
  /// files.
  Read { head: Revision, diff: String },
  /// Larger than Orchester reads; only its size was read.
  TooLarge { head: Revision, size: Changes },
  /// Git could not read it.
  Failed(WorktreeReadFailure),
}

/// Why git could not read a run's worktree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum WorktreeReadFailure {
  /// No git program was found.
  #[error("git was not found")]
  GitMissing,
  /// Git could not run, failed, or printed something else than expected, as `detail` says, without credentials.
  #[error("git could not read the worktree: {detail}")]
  Git { detail: String },
}
