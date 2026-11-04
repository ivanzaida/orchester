//! What a run's folder holds beyond its last commit, as the runner reads it for the desktop's start-up recovery
//! ([`crate::Command::RunFolderState`], protocol 43): the desktop reaches a run's worktree only through the runner
//! (`docs/architecture/remote-runners.md`, rule 5).

use serde::{Deserialize, Serialize};

/// The state of a run's folder: its own worktree, else the workspace it holds or worked in last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FolderState {
  /// Nothing beyond its last commit.
  Clean,
  /// `files` files are changed or untracked; `first` names the first of them as a repository path, when git's line
  /// for it can be read.
  Uncommitted { files: usize, first: Option<String> },
  /// The run has no folder any more, or git could not tell.
  Unknown,
}
