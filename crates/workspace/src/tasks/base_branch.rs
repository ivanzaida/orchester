//! A task's base branch as a person sets it on New task and Edit task (`Base branch · redesign`, `ri5ru`, 01 `eUFSd`):
//! inherit it from the task's module, take the branch checked out when a run starts, or one of the repository's
//! branches; a remote-tracking branch with no local one creates its local branch, tracking it, once the task is saved.
//! Kept on this computer only (`orchester_core::integration::base`): a shared task's is never sent to its server.

use std::path::Path;

use orchester_core::{
  id::TaskId,
  integration::{
    GitError,
    base::{BranchChoices, BranchPick, BranchPickError, TaskBase, branch_choices, take_branch},
    checked_out_branch,
  },
  run::BranchName,
  store::TaskStore,
  task::{Task, TaskUpdate},
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::{changes::GIT, integration::git_message};
use crate::messages::{self, Message};

/// A task's base branch as the person picked it: a branch picked among the repository's is taken only when the task
/// is saved ([`set_task_base`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BasePick {
  #[default]
  Inherit,
  CheckedOut,
  Branch(BranchPick),
}

impl BasePick {
  /// The pick that keeps `base`.
  pub fn of(base: &TaskBase) -> Self {
    match base {
      TaskBase::Inherit => Self::Inherit,
      TaskBase::CheckedOut => Self::CheckedOut,
      TaskBase::Branch(branch) => Self::Branch(BranchPick::Local(branch.clone())),
    }
  }

  /// The local branch the pick names, or creates; `None` when it inherits or takes the checked-out branch.
  pub const fn branch(&self) -> Option<&BranchName> {
    match self {
      Self::Inherit | Self::CheckedOut => None,
      Self::Branch(BranchPick::Local(branch)) => Some(branch),
      Self::Branch(BranchPick::Remote(remote)) => Some(remote.local()),
    }
  }

  /// Whether saving the pick leaves `base` as it is.
  pub fn keeps(&self, base: &TaskBase) -> bool {
    match (self, base) {
      (Self::Inherit, TaskBase::Inherit) | (Self::CheckedOut, TaskBase::CheckedOut) => true,
      (Self::Branch(BranchPick::Local(picked)), TaskBase::Branch(branch)) => picked == branch,
      _ => false,
    }
  }
}

/// What the Base branch menu offers: the repository's branches, and the branch checked out now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BranchMenu {
  pub choices: BranchChoices,
  /// `None` on a detached HEAD.
  pub checked_out: Option<BranchName>,
}

/// The branches of the repository at `root` a task's base branch can be set to, and the one checked out now.
pub fn branch_menu(programs: &Programs, root: &Path) -> Result<BranchMenu, Message> {
  let read = || -> Result<BranchMenu, GitError> {
    Ok(BranchMenu {
      choices: branch_choices(&programs.runner(), Path::new(GIT), root)?,
      checked_out: checked_out_branch(&programs.runner(), Path::new(GIT), root)?,
    })
  };
  read().map_err(|error| {
    tracing::warn!(%error, "cannot read the repository's branches");
    git_message(&error)
  })
}

/// Sets the base branch of `task`, saved just now in the repository at `root`, as `pick` says, unless it has it
/// already: a remote-tracking branch creates its local branch first; a local branch that is gone is refused. The task
/// as it is stored then.
pub fn set_task_base(programs: &Programs, root: &Path, task: Task, pick: &BasePick) -> Result<Task, Message> {
  if pick.keeps(task.base()) {
    return Ok(task);
  }
  let base = match pick {
    BasePick::Inherit => TaskBase::Inherit,
    BasePick::CheckedOut => TaskBase::CheckedOut,
    BasePick::Branch(picked) => TaskBase::Branch(
      take_branch(&programs.git_work(), Path::new(GIT), (root, picked)).map_err(|error| match error {
        BranchPickError::Gone(branch) => Message::new("tasks.baseBranch.gone").with("branch", branch.to_string()),
        BranchPickError::Git(error) => {
          tracing::warn!(%error, "cannot take the base branch picked");
          git_message(&error)
        }
      })?,
    ),
  };
  stored(root, task.id(), &base).map_err(|error| {
    tracing::error!(%error, "cannot store the task's base branch");
    messages::store_error(&error)
  })?
}

/// Stores `base` on the task `id` as it reads now; `Err` inside when it is gone, or changed between that read and the
/// write. Only the base columns are written, so a change made since the save keeps its other fields.
fn stored(root: &Path, id: TaskId, base: &TaskBase) -> Result<Result<Task, Message>, orchester_store::StoreError> {
  let mut db = ProjectDb::open(root)?;
  let Some(read) = db.task(id)? else {
    return Ok(Err(Message::new("tasks.baseBranch.changedMeanwhile")));
  };
  Ok(match db.set_task_base(id, read.revision(), base)? {
    TaskUpdate::Saved(task) => Ok(task),
    TaskUpdate::Conflict { .. } => Err(Message::new("tasks.baseBranch.changedMeanwhile")),
  })
}
