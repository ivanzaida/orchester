//! The desktop's requests about the folders a project's runs leave with their executor (protocol 42,
//! `docs/architecture/remote-runners.md`, rule 5): the warm workspaces and the worktrees runs made before workspaces.
//! The desktop keeps the project database's side (reserving a workspace, recording it cold or no longer stranded);
//! the runner's executor side sizes the folders and changes them with git.

use orchester_core::{id::RunId, run::BranchName, workspaces::WorkspaceNumber};
use serde::{Deserialize, Serialize};

/// What the desktop asks of the executor's folders ([`crate::Command::Folders`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum FoldersCommand {
  /// The sizes of warm workspaces `workspaces`, and every worktree a run made before workspaces with its size.
  /// Answered with [`FoldersReply::Sizes`].
  Sizes { workspaces: Vec<WorkspaceNumber> },
  /// Deletes every ignored file of idle workspace `number`, its warm folders too, which the desktop reserved.
  /// Answered with [`FoldersReply::Cleaned`].
  Clean { number: WorkspaceNumber },
  /// "Discard": throws away the uncommitted changes of stranded workspace `number`, which the desktop reserved.
  /// Answered with [`FoldersReply::Discarded`].
  Discard { number: WorkspaceNumber },
  /// "Keep them in a branch": commits the uncommitted changes of stranded workspace `number`, which the desktop
  /// reserved, on the new branch `branch`. Answered with [`FoldersReply::Kept`].
  Keep {
    number: WorkspaceNumber,
    branch: BranchName,
  },
}

/// The answer to a [`FoldersCommand`] ([`crate::Reply::Folders`]); a git or file error is a
/// [`crate::Rejection::Failed`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum FoldersReply {
  Sizes(FolderSizes),
  /// The bytes the cleaning freed.
  Cleaned {
    freed: u64,
  },
  Discarded,
  Kept,
}

/// What the folders hold on disk; `None` for one that could not be read.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderSizes {
  /// Each workspace asked about, in the order asked.
  pub workspaces: Vec<(WorkspaceNumber, Option<u64>)>,
  /// Each worktree a run made before workspaces that is still there, in no certain order.
  pub run_worktrees: Vec<(RunId, Option<u64>)>,
}
