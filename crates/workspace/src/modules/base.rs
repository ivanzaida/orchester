//! A module's base branch (`Base branch · redesign`, 02 `eyrnI`): what its chip's menu offers, the branch the checkout
//! has now and the repository's branches, with how many of the module's tasks take it, and the save of the one chosen.
//! Kept on this computer only, as `orchester_core::integration::base` says. Blocks on SQLite and git; the UI calls it
//! off the UI thread.

use std::path::Path;

use orchester_core::{
  id::ModuleId,
  integration::{
    base::{BaseChoice, BranchChoices, BranchPick, BranchPickError, TaskBase, branch_choices, take_branch},
    checked_out_branch,
  },
  run::BranchName,
  store::TaskStore,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use crate::{
  messages::{self, Message},
  tasks::{changes::GIT, integration::git_message},
};

/// What the menu of a module's Base branch chip offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleBaseRead {
  /// The branch the checkout has checked out now; `None` on a detached `HEAD`.
  pub checked_out: Option<BranchName>,
  pub choices: BranchChoices,
  /// How many of the module's open tasks have no base branch of their own and run from its; none for a new module.
  pub inheriting: usize,
}

/// A base branch a person chooses for a module.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ModuleBasePick {
  /// The branch the checkout has checked out when each run starts.
  #[default]
  CheckedOut,
  Branch(BranchPick),
}

impl ModuleBasePick {
  /// The pick that keeps `base`.
  pub fn of(base: &BaseChoice) -> Self {
    base.branch().map_or(Self::CheckedOut, |branch| {
      Self::Branch(BranchPick::Local(branch.clone()))
    })
  }

  /// The branch the module runs from once the pick is saved; `None` for the checked-out branch.
  pub const fn branch(&self) -> Option<&BranchName> {
    match self {
      Self::CheckedOut => None,
      Self::Branch(BranchPick::Local(branch)) => Some(branch),
      Self::Branch(BranchPick::Remote(remote)) => Some(remote.local()),
    }
  }
}

/// The menu's choices for module `module` (none for a new one) of the project at `root`.
pub fn module_base_in(root: &Path, programs: &Programs, module: Option<ModuleId>) -> Result<ModuleBaseRead, Message> {
  let inheriting = match module {
    Some(module) => ProjectDb::open(root)
      .and_then(|db| db.tasks())
      .map_err(|error| messages::store_error(&error))?
      .iter()
      .filter(|task| {
        task.module() == Some(module) && *task.base() == TaskBase::Inherit && !task.organization().status().is_closed()
      })
      .count(),
    None => 0,
  };
  let runner = programs.runner();
  let read_failed = |error| {
    tracing::warn!(%error, "cannot list the repository's branches");
    git_message(&error)
  };
  let checked_out = checked_out_branch(&runner, Path::new(GIT), root).map_err(read_failed)?;
  let choices = branch_choices(&runner, Path::new(GIT), root).map_err(read_failed)?;
  Ok(ModuleBaseRead {
    checked_out,
    choices,
    inheriting,
  })
}

/// The base branch `pick` names in the repository at `root`, before anything is saved: a remote-tracking branch
/// creates its local branch, unless the repository has it by now; a local branch that is gone is refused.
pub fn take_module_base_in(root: &Path, programs: &Programs, pick: &ModuleBasePick) -> Result<BaseChoice, Message> {
  let ModuleBasePick::Branch(pick) = pick else {
    return Ok(BaseChoice::CheckedOut);
  };
  match take_branch(&programs.git_work(), Path::new(GIT), (root, pick)) {
    Ok(branch) => Ok(BaseChoice::Branch(branch)),
    Err(BranchPickError::Gone(branch)) => {
      Err(Message::new("modules.baseBranchGone").with("branch", branch.to_string()))
    }
    Err(BranchPickError::Git(error)) => {
      tracing::warn!(%error, "cannot take the module's base branch");
      Err(git_message(&error))
    }
  }
}

/// Sets the base branch of module `module`, read as `before`, to `after`; a change made meanwhile is refused.
pub fn save_module_base_in(
  root: &Path,
  module: ModuleId,
  (before, after): (&BaseChoice, BaseChoice),
) -> Result<(), Message> {
  ProjectDb::open(root)
    .and_then(|mut db| db.set_module_base(module, before, after))
    .map(drop)
    .map_err(|error| messages::store_error(&error))
}
