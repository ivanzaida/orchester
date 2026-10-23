//! The project's base branch (Project settings › General › *Base branch*, ORC-81): the branch its new runs start
//! from, and so the one they deliver into. By default it is the branch the person's checkout has checked out when a
//! run starts, as before the setting existed; set, it is a branch of the repository, whose tip a new run starts from
//! wherever the checkout is. A run records the branch it started from (`runs.base_branch`) and every delivery follows
//! that record, so a change of the setting reaches only the runs that start after it. A project whose base branch is
//! set lands a run beside the checkout while the checkout has another branch ([`Landing`]).

use std::path::Path;

use thiserror::Error;

use super::{GitError, git::Git};
use crate::{command::CommandRunner, revision::Revision, run::BranchName};

/// Where the project's new runs start from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BaseBranchSetting {
  /// The branch the checkout has checked out when a run starts, at its `HEAD`.
  #[default]
  CheckedOut,
  /// This branch, at its tip, wherever the checkout is.
  Branch(BranchName),
}

impl BaseBranchSetting {
  /// The branch set; `None` for the checked-out branch.
  pub const fn branch(&self) -> Option<&BranchName> {
    match self {
      Self::CheckedOut => None,
      Self::Branch(branch) => Some(branch),
    }
  }
}

/// A remote-tracking branch with no local branch of its name: choosing it as the base branch creates that local
/// branch, tracking it ([`create_tracking_branch`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteBranch {
  /// As git names it short: `origin/staging`.
  tracking: String,
  local: BranchName,
}

impl RemoteBranch {
  /// The remote-tracking branch, as git names it short (`origin/staging`).
  pub fn tracking(&self) -> &str {
    &self.tracking
  }

  /// The local branch choosing it creates (`staging`).
  pub const fn local(&self) -> &BranchName {
    &self.local
  }
}

/// The branches the base branch can be set to: the repository's local branches, and the remote-tracking ones that
/// have no local branch of their name, each in git's order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BranchChoices {
  pub local: Vec<BranchName>,
  pub remote: Vec<RemoteBranch>,
}

impl BranchChoices {
  /// Whether the repository has the local branch `branch`.
  pub fn has_local(&self, branch: &BranchName) -> bool {
    self.local.contains(branch)
  }
}

/// The branches of the repository at `repo` the base branch can be set to. A remote's `HEAD` is not a branch.
pub fn branch_choices(runner: &impl CommandRunner, git: &Path, repo: &Path) -> Result<BranchChoices, GitError> {
  let git = Git::new(runner, git, repo);
  let remotes: Vec<String> = git.stdout(&["remote"])?.lines().map(str::to_owned).collect();
  let listed = git.stdout(&["for-each-ref", "--format=%(refname)", "refs/heads", "refs/remotes"])?;
  let mut choices = BranchChoices::default();
  let mut tracking = Vec::new();
  for name in listed.lines() {
    if let Some(local) = name.strip_prefix("refs/heads/") {
      choices.local.extend(BranchName::new(local).ok());
    } else if let Some(remote) = name.strip_prefix("refs/remotes/") {
      tracking.extend(remote_branch(&remotes, remote));
    }
  }
  choices.remote = tracking
    .into_iter()
    .filter(|remote: &RemoteBranch| !choices.has_local(&remote.local))
    .collect();
  Ok(choices)
}

/// `remote/branch`, a remote-tracking ref without `refs/remotes/`, as the branch of the longest remote of `remotes`
/// it starts with; `None` for a remote's `HEAD`, a ref of no remote or a name a branch cannot have.
fn remote_branch(remotes: &[String], name: &str) -> Option<RemoteBranch> {
  let branch = remotes
    .iter()
    .filter_map(|remote| name.strip_prefix(remote.as_str())?.strip_prefix('/'))
    .min_by_key(|branch| branch.len())?;
  if branch == "HEAD" {
    return None;
  }
  Some(RemoteBranch {
    tracking: name.to_owned(),
    local: BranchName::new(branch).ok()?,
  })
}

/// Creates the local branch of `remote` at its tip, tracking it, in the repository at `repo`; the checkout stays on
/// its branch.
pub fn create_tracking_branch(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, remote): (&Path, &RemoteBranch),
) -> Result<(), GitError> {
  Git::new(runner, git, repo)
    .stdout(&[
      "branch",
      "--quiet",
      "--track",
      "--end-of-options",
      remote.local.as_str(),
      &format!("refs/remotes/{}", remote.tracking),
    ])
    .map(drop)
}

/// Why a new run does not start on the base branch set.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RunBaseError {
  /// The branch set is gone: the run is refused, and does not start from the checkout instead.
  #[error("the base branch {0} does not exist")]
  Missing(BranchName),
  #[error(transparent)]
  Git(#[from] GitError),
}

/// The commit a new run starts from on `branch`, the base branch set: its tip in the repository at `repo`.
pub fn set_branch_tip(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, branch): (&Path, &BranchName),
) -> Result<Revision, RunBaseError> {
  super::branch_head(runner, git, (repo, branch))?.ok_or_else(|| RunBaseError::Missing(branch.clone()))
}

/// Where merging a run may land it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Landing {
  /// Only in the checkout, which must have the run's base branch checked out: a project that follows the
  /// checked-out branch, as before the setting existed.
  Checkout,
  /// In the checkout while it has the run's base branch checked out; else beside it, on the branch itself, without
  /// switching or touching the checkout: a project whose base branch is set.
  BesideOtherBranch,
}

impl Landing {
  /// How a project with the base branch `setting` lands its runs.
  pub const fn of(setting: &BaseBranchSetting) -> Self {
    match setting {
      BaseBranchSetting::CheckedOut => Self::Checkout,
      BaseBranchSetting::Branch(_) => Self::BesideOtherBranch,
    }
  }

  /// Whether a run whose base branch is `into` lands beside a checkout on `checked_out` (`None`: a detached HEAD).
  pub fn beside(self, into: &BranchName, checked_out: Option<&BranchName>) -> bool {
    self == Self::BesideOtherBranch && checked_out != Some(into)
  }
}
