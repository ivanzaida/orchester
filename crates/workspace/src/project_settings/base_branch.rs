//! Project settings › General › *Base branch* (ORC-81, `orchester_core::integration::base`): the branch the project's
//! new runs start from and deliver into, as the window and automation read and set it. Kept in the project's
//! database on this computer only. Blocks on SQLite and git; the UI calls it off the UI thread.

use std::path::Path;

use orchester_core::{
  integration::base::{
    BaseBranchSetting, BranchChoices, Landing, RemoteBranch, branch_choices, create_tracking_branch,
  },
  run::BranchName,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use crate::{
  messages::{self, Message},
  tasks::{changes::GIT, integration::git_message},
};

/// The setting and the branches it can be set to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseBranchRead {
  pub setting: BaseBranchSetting,
  pub choices: BranchChoices,
}

impl BaseBranchRead {
  /// The branch set, when the repository no longer has it: new runs are refused until another is chosen.
  pub fn missing(&self) -> Option<&BranchName> {
    self.setting.branch().filter(|branch| !self.choices.has_local(branch))
  }
}

/// What a person chooses for the setting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BaseBranchChoice {
  CheckedOut,
  Local(BranchName),
  /// A remote-tracking branch, whose local branch the choice creates.
  Remote(RemoteBranch),
}

/// A choice saved: the setting now, and how many runs keep the base branch they recorded (none when the setting did
/// not change).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseBranchSaved {
  pub setting: BaseBranchSetting,
  pub kept: usize,
}

/// The project's base branch at `root` and the repository's branches.
pub fn base_branch_in(root: &Path, programs: &Programs) -> Result<BaseBranchRead, Message> {
  let setting = ProjectDb::open(root)
    .and_then(|db| db.base_branch_setting())
    .map_err(|error| messages::store_error(&error))?;
  let choices = branch_choices(&programs.runner(), Path::new(GIT), root).map_err(|error| {
    tracing::warn!(%error, "cannot list the repository's branches");
    git_message(&error)
  })?;
  Ok(BaseBranchRead { setting, choices })
}

/// Sets the project's base branch at `root` to `choice`; a remote-tracking branch creates its local branch first,
/// unless the repository has it by now. A local branch that is gone is refused, nothing saved.
pub fn save_base_branch_in(
  root: &Path,
  programs: &Programs,
  choice: &BaseBranchChoice,
) -> Result<BaseBranchSaved, Message> {
  let store = |error: StoreError| messages::store_error(&error);
  let choices = branch_choices(&programs.runner(), Path::new(GIT), root).map_err(|error| git_message(&error))?;
  let setting = match choice {
    BaseBranchChoice::CheckedOut => BaseBranchSetting::CheckedOut,
    BaseBranchChoice::Local(branch) if choices.has_local(branch) => BaseBranchSetting::Branch(branch.clone()),
    BaseBranchChoice::Local(branch) => {
      return Err(Message::new("projectSettings.baseBranch.gone").with("branch", branch.to_string()));
    }
    BaseBranchChoice::Remote(remote) => {
      if !choices.has_local(remote.local()) {
        create_tracking_branch(&programs.git_work(), Path::new(GIT), (root, remote)).map_err(|error| {
          tracing::warn!(%error, tracking = remote.tracking(), "cannot create the base branch from its remote");
          git_message(&error)
        })?;
      }
      BaseBranchSetting::Branch(remote.local().clone())
    }
  };
  let mut db = ProjectDb::open(root).map_err(store)?;
  let before = db.base_branch_setting().map_err(store)?;
  db.save_base_branch_setting(&setting).map_err(store)?;
  let kept = if before == setting {
    0
  } else {
    db.runs_keeping_their_base().map_err(store)?
  };
  Ok(BaseBranchSaved { setting, kept })
}

/// How the project in `db` lands its runs' merges ([`Landing`]).
pub fn landing(db: &ProjectDb) -> Result<Landing, StoreError> {
  db.base_branch_setting().map(|setting| Landing::of(&setting))
}
