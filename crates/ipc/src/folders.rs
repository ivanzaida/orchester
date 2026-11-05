//! The desktop's requests about the folders a project's runs leave with their executor (protocol 42,
//! `docs/architecture/remote-runners.md`, rule 5): the warm workspaces and the worktrees runs made before workspaces.
//! The desktop keeps the project database's side (reserving a workspace, recording it cold or no longer stranded);
//! the runner's executor side lists and sizes the folders and changes them with git. Listing without sizing and
//! sizing one folder came with protocol 51, for Storage's inventory, which measures its rows one by one.

use orchester_core::{run::BranchName, workspaces::WorkspaceNumber};
use serde::{Deserialize, Serialize};

/// What the desktop asks of the executor's folders ([`crate::Command::Folders`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum FoldersCommand {
  /// The sizes of warm workspaces `workspaces`, and every worktree a run made before workspaces with its size.
  /// Answered with [`FoldersReply::Sizes`].
  Sizes { workspaces: Vec<WorkspaceNumber> },
  /// Which of warm workspaces `workspaces` have a folder, and the names of the worktrees folder's folders, none
  /// sized. Answered with [`FoldersReply::Listed`]; a worktrees folder that is not there lists none, one that cannot
  /// be listed is a [`crate::Rejection::Failed`].
  List { workspaces: Vec<WorkspaceNumber> },
  /// The size of one folder. Answered with [`FoldersReply::Measured`]; a worktree name that is not one folder of
  /// the worktrees folder is a [`crate::Rejection::Failed`].
  Measure { folder: ExecutorFolder },
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
  Listed(FolderList),
  /// The folder's size; `None` when it could not be read.
  Measured {
    bytes: Option<u64>,
  },
}

/// What the folders hold on disk; `None` for one that could not be read.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderSizes {
  /// Each workspace asked about, in the order asked.
  pub workspaces: Vec<(WorkspaceNumber, Option<u64>)>,
  /// Each folder of the worktrees folder that is still there, by its name, in no certain order: a run's own worktree
  /// is named by its run's id, a step execution's by the execution's id.
  pub worktrees: Vec<(String, Option<u64>)>,
}

/// The folders that are there, not sized ([`FoldersCommand::List`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderList {
  /// Each workspace asked about whose folder is there, in the order asked.
  pub workspaces: Vec<WorkspaceNumber>,
  /// The name of each folder of the worktrees folder, in no certain order, named as in [`FolderSizes::worktrees`].
  pub worktrees: Vec<String>,
}

/// One folder of the executor's ([`FoldersCommand::Measure`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutorFolder {
  /// The worktrees folder's folder `name`, as [`FolderList::worktrees`] names it.
  Worktree { name: String },
  /// Warm workspace `number`'s folder.
  Workspace { number: WorkspaceNumber },
}
