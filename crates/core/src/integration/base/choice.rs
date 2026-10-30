//! Where a run's base comes from (`Base branch · redesign`, `ri5ru`): a task's base branch, else its module's, else
//! the branch the checkout has checked out when the run starts. The run records the branch it took and where it came
//! from ([`RunBaseSource`]), so a later change of a task's or module's base branch reaches only the runs that start
//! after it.

use serde::{Deserialize, Serialize};

use crate::{id::ModuleId, run::BranchName};

/// A branch a run starts from: the one the checkout has checked out when it starts, or a branch of the repository.
/// A module's base branch is one of these, the checked-out branch by default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BaseChoice {
  /// The branch the checkout has checked out when a run starts, at its `HEAD`.
  #[default]
  CheckedOut,
  /// This branch, at its tip, wherever the checkout is.
  Branch(BranchName),
}

impl BaseChoice {
  /// The branch set; `None` for the checked-out branch.
  pub const fn branch(&self) -> Option<&BranchName> {
    match self {
      Self::CheckedOut => None,
      Self::Branch(branch) => Some(branch),
    }
  }
}

/// A task's base branch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TaskBase {
  /// Its module's base branch; for a task in no module, the checked-out branch.
  #[default]
  Inherit,
  /// The checked-out branch, whatever its module says.
  CheckedOut,
  Branch(BranchName),
}

impl TaskBase {
  /// The branch set on the task itself; `None` when it inherits or takes the checked-out branch.
  pub const fn branch(&self) -> Option<&BranchName> {
    match self {
      Self::Inherit | Self::CheckedOut => None,
      Self::Branch(branch) => Some(branch),
    }
  }
}

/// Where a run's base branch came from, recorded with the run when it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RunBaseSource {
  /// The branch set on the task itself.
  Task,
  /// The branch set on the module the task was in.
  Module { module: ModuleId },
  /// No branch was set for the task: the branch checked out when the run started.
  CheckedOut,
  /// The project's base branch, for runs recorded before tasks and modules had theirs; never produced again.
  Project,
}

impl RunBaseSource {
  /// Whether the run took a branch set for it, rather than whatever the checkout had checked out.
  pub const fn set(self) -> bool {
    match self {
      Self::Task | Self::Module { .. } | Self::Project => true,
      Self::CheckedOut => false,
    }
  }
}

/// The base a new run of a task with base branch `task` takes, in `module` with its base branch (none for a task in
/// no module), and where it came from. A task that chose the checked-out branch takes it whatever its module says;
/// one that inherits from a module without a branch, or from no module, takes it too.
pub fn resolve(task: &TaskBase, module: Option<(ModuleId, &BaseChoice)>) -> (BaseChoice, RunBaseSource) {
  match (task, module) {
    (TaskBase::Branch(branch), _) => (BaseChoice::Branch(branch.clone()), RunBaseSource::Task),
    (TaskBase::Inherit, Some((module, BaseChoice::Branch(branch)))) => {
      (BaseChoice::Branch(branch.clone()), RunBaseSource::Module { module })
    }
    (TaskBase::CheckedOut | TaskBase::Inherit, _) => (BaseChoice::CheckedOut, RunBaseSource::CheckedOut),
  }
}
