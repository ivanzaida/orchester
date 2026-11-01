//! A run's base branch: the branch it starts from, and so the one it delivers into. A task's base branch, else its
//! module's, else the branch the person's checkout has checked out when the run starts ([`resolve`]); a branch set is
//! a branch of the repository, whose tip a new run starts from wherever the checkout is. A run records the branch it
//! started from and where it came from (`runs.base_branch`, `runs.base_source`) and every delivery follows that
//! record, so a change of a task's or module's base branch reaches only the runs that start after it. A run whose
//! base branch was set lands beside the checkout while the checkout has another branch ([`Landing`]).

mod choice;

use std::path::Path;

use thiserror::Error;

pub use self::choice::{BaseChoice, RunBaseSource, TaskBase, resolve};
use super::{GitError, git::Git};
use crate::{command::CommandRunner, revision::Revision, run::BranchName};

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

/// A branch a person picks as a task's or a module's base branch among the repository's ([`branch_choices`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BranchPick {
  Local(BranchName),
  /// A remote-tracking branch, whose local branch the pick creates.
  Remote(RemoteBranch),
}

/// Why a branch picked is not taken.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BranchPickError {
  /// The local branch picked is gone.
  #[error("the branch {0} does not exist")]
  Gone(BranchName),
  #[error(transparent)]
  Git(#[from] GitError),
}

/// The branch `pick` names in the repository at `repo`: a remote-tracking one creates its local branch first, unless
/// the repository has it by now; a local branch that is gone is refused.
pub fn take_branch(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, pick): (&Path, &BranchPick),
) -> Result<BranchName, BranchPickError> {
  let choices = branch_choices(runner, git, repo)?;
  match pick {
    BranchPick::Local(branch) if choices.has_local(branch) => Ok(branch.clone()),
    BranchPick::Local(branch) => Err(BranchPickError::Gone(branch.clone())),
    BranchPick::Remote(remote) => {
      if !choices.has_local(&remote.local) {
        create_tracking_branch(runner, git, (repo, remote))?;
      }
      Ok(remote.local.clone())
    }
  }
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
  /// Only in the checkout, which must have the run's base branch checked out: a run that took the checked-out
  /// branch.
  Checkout,
  /// In the checkout while it has the run's base branch checked out; else beside it, on the branch itself, without
  /// switching or touching the checkout: a run whose base branch was set.
  BesideOtherBranch,
}

impl Landing {
  /// How a run whose base branch came from `source` lands.
  pub const fn of(source: RunBaseSource) -> Self {
    if source.set() {
      Self::BesideOtherBranch
    } else {
      Self::Checkout
    }
  }

  /// Whether a run whose base branch is `into` lands beside a checkout on `checked_out` (`None`: a detached HEAD).
  pub fn beside(self, into: &BranchName, checked_out: Option<&BranchName>) -> bool {
    self == Self::BesideOtherBranch && checked_out != Some(into)
  }
}
