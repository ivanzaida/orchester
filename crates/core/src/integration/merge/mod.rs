//! Merging a run that succeeded into the base branch it recorded when it started, in the user's
//! checkout. It merges only a checkout on that branch with no merge, cherry-pick or revert in
//! progress, and never stashes, resets or touches uncommitted work: uncommitted changes and
//! untracked files the merge does not reach stay as they are. It fast-forwards when it can, else
//! commits a merge ("Merge ORC-12 run #3") with the repository's git identity in a temporary
//! worktree ([`temporary`]), where a conflict or a hook that refuses the merge commit stops it
//! without reaching the checkout. Either result reaches the checkout as a fast-forward
//! ([`bring_in`]), which git refuses, changing nothing, when it would overwrite or remove a file
//! with uncommitted changes, an untracked one or an ignored one; the refusal names exactly those.
//! The repository's hooks run: they are the user's. A project whose base branch is set lands a run
//! beside a checkout on another branch instead ([`beside`]).

mod beside;
mod bring_in;
mod restore;
mod temporary;

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{GitError, base::Landing, git::Git};
use crate::{
  command::CommandRunner,
  id::RunId,
  project::TaskKey,
  revision::Revision,
  run::{BranchName, Run, RunStatus},
  time::Timestamp,
};

/// Refs that mean an operation is in progress in the checkout.
const IN_PROGRESS: [&str; 3] = ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD"];

/// How the run's work reached the base branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeKind {
  /// The base branch moved to the run's branch.
  FastForward,
  /// A merge commit joined both.
  MergeCommit,
  /// The base branch already held every commit of the run's branch; nothing changed.
  AlreadyMerged,
  /// One commit on the base branch holds the run's work squashed (a Deliver step's *Squash*); the run's branch is
  /// not part of the base branch's history.
  Squash,
}

/// How the merge queue lands a run as its pipeline's Deliver step asks: always a merge commit, or one squashed commit
/// with its `title` and `body`. A merge from the task page fast-forwards when it can instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveredAs<'a> {
  MergeCommit,
  Squash { title: &'a str, body: &'a str },
}

/// What the merge queue lands: `as_delivered`, onto the base branch only while it is at `base`, the head the run was
/// synced and checked on. A base that moved since, by anyone, merges nothing ([`MergeError::BaseMoved`]): a run is
/// never merged on a check of an older base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delivering<'a> {
  pub as_delivered: DeliveredAs<'a>,
  pub base: &'a Revision,
}

/// A run whose work was merged: into which branch, the commit that branch was at afterwards, and
/// when. Written once, by whoever merged it (the desktop).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunMerge {
  pub run: RunId,
  pub into: BranchName,
  pub revision: Revision,
  pub kind: MergeKind,
  pub merged_at: Timestamp,
}

/// A merge that happened, and the temporary worktree of its merge commit when that could not be
/// removed: the merge stands either way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeOutcome {
  pub merge: RunMerge,
  pub left: Option<TemporaryLeft>,
}

/// The folder of a temporary worktree that stayed, and why: git's message, or the system's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemporaryLeft {
  pub path: String,
  pub detail: String,
}

/// Why a run was not merged. Except for [`MergeError::LeftChanged`] and for [`MergeError::Git`]
/// while the checkout was being brought to the merge, nothing in the checkout changed.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MergeError {
  #[error("only a run that succeeded can be merged")]
  NotSucceeded,
  /// The run started on a detached HEAD, or before Orchester recorded the branch.
  #[error("the run did not record the branch it started from")]
  NoBaseBranch,
  /// `current` is `None` on a detached HEAD. Checked before the merge and again before it reaches the checkout.
  #[error("the checkout is on {current:?}, not {expected}")]
  WrongBranch {
    expected: BranchName,
    current: Option<BranchName>,
  },
  /// Bringing the merge into the checkout would overwrite or remove these files, which have
  /// uncommitted changes there or are untracked or ignored. Uncommitted work elsewhere is no reason.
  #[error("the merge would overwrite uncommitted changes in the checkout: {files:?}")]
  UncommittedInTheWay { files: Vec<String> },
  #[error("a merge, cherry-pick or revert is in progress in the checkout")]
  InProgress,
  #[error("the run's branch {0} does not exist")]
  BranchMissing(BranchName),
  /// The base branch the run would land beside the checkout is gone.
  #[error("the base branch {0} does not exist")]
  BaseMissing(BranchName),
  /// The base branch the run would land beside the checkout is checked out in the worktree at `path`, as git
  /// prints it: moving it would leave that worktree's files behind.
  #[error("the base branch {branch} is checked out in {path}")]
  CheckedOutIn { branch: BranchName, path: String },
  /// The run's branch is no longer at the commit the merge was asked for, e.g. the one a person reviewed.
  #[error("the run's branch {branch} is no longer at {expected}")]
  BranchMoved { branch: BranchName, expected: Revision },
  /// The base branch got a commit while the merge commit was made; the merge was dropped.
  #[error("{branch} moved while the run was merged")]
  BaseMoved { branch: BranchName },
  /// Both sides changed these files; the merge was undone.
  #[error("the run's branch conflicts with the base branch in {files:?}")]
  Conflict { files: Vec<String> },
  /// Git did not merge, e.g. a hook refused the merge commit; the merge was undone. Its message.
  #[error("git did not merge: {0}")]
  Refused(String),
  /// Git stopped part way through bringing the merge in, and these files it had written could not be
  /// put back: they hold the run's version. `detail` says why they could not.
  #[error("git stopped part way and {files:?} could not be put back: {detail}")]
  LeftChanged { files: Vec<String>, detail: String },
  #[error(transparent)]
  Git(#[from] GitError),
}

/// Merges the branch of `run`, a run of task `task` that succeeded, into its base branch in the
/// user's checkout at `repo`, with the `git` program; the merge is recorded at `now`.
pub fn merge_run(
  runner: &impl CommandRunner,
  git: &Path,
  repo: &Path,
  (run, task): (&Run, &TaskKey),
  now: Timestamp,
) -> Result<MergeOutcome, MergeError> {
  merge_run_at(runner, (git, repo), (run, task), None, now)
}

/// [`merge_run`], only while the run's branch is at `expected` when it names one: a branch that moved since is
/// refused ([`MergeError::BranchMoved`]) and nothing is merged.
pub fn merge_run_at(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  (run, task): (&Run, &TaskKey),
  expected: Option<&Revision>,
  now: Timestamp,
) -> Result<MergeOutcome, MergeError> {
  merge_run_as(
    runner,
    (git, repo),
    (run, task),
    (expected, None, Landing::Checkout),
    now,
  )
}

/// [`merge_run_at`], landed as `delivering` says when the merge queue asks (`None`: fast-forwarding when it can, as
/// the task page merges), where `landing` lets it: beside a checkout on another branch for a run whose base branch
/// was set.
pub fn merge_run_as(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  (run, task): (&Run, &TaskKey),
  (expected, delivering, landing): (Option<&Revision>, Option<Delivering<'_>>, Landing),
  now: Timestamp,
) -> Result<MergeOutcome, MergeError> {
  let (revision, kind, left) = merge(runner, (git, repo), (run, task), (expected, delivering, landing))?;
  let merge = RunMerge {
    run: run.id(),
    into: run.base_branch().ok_or(MergeError::NoBaseBranch)?.clone(),
    revision,
    kind,
    merged_at: now,
  };
  Ok(MergeOutcome { merge, left })
}

type Merged = (Revision, MergeKind, Option<TemporaryLeft>);

fn merge(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  (run, task): (&Run, &TaskKey),
  (expected, delivering, landing): (Option<&Revision>, Option<Delivering<'_>>, Landing),
) -> Result<Merged, MergeError> {
  if run.status() != &RunStatus::Succeeded {
    return Err(MergeError::NotSucceeded);
  }
  let into = run.base_branch().ok_or(MergeError::NoBaseBranch)?;
  let git = Git::new(runner, git, repo);
  if landing.beside(into, git.checked_out_branch()?.as_ref()) {
    let head = run_head(&git, run, expected)?;
    return beside::merge(&git, (run, task), (into, &head), delivering);
  }
  check_checkout(&git, into)?;
  let head = run_head(&git, run, expected)?;
  let base = git
    .commit("HEAD")?
    .ok_or_else(|| GitError::Failed("HEAD names no commit".to_owned()))?;
  if let Some(delivering) = delivering
    && base != delivering.base.as_str()
  {
    return Err(MergeError::BaseMoved { branch: into.clone() });
  }
  if git.is_ancestor(&head, &base)? {
    return Ok((revision(&base)?, MergeKind::AlreadyMerged, None));
  }
  if delivering.is_none() && git.is_ancestor(&base, &head)? {
    bring_in::bring_in(&git, (&base, &head))?;
    return Ok((revision(&head)?, MergeKind::FastForward, None));
  }
  let title = title(run, task);
  let (made, kind) = made(&title, delivering);
  let (merged, left) = temporary::merge_commit(&git, run.id(), (&base, &head), made)?;
  // Hooks ran meanwhile, and so may the person: the checkout must still be where the merge began.
  check_checkout(&git, into)?;
  if git.commit("HEAD")?.as_deref() != Some(base.as_str()) {
    return Err(MergeError::BaseMoved { branch: into.clone() });
  }
  bring_in::bring_in(&git, (&base, &merged))?;
  Ok((revision(&merged)?, kind, left))
}

/// The tip of `run`'s branch, while it is at `expected` when that names one.
fn run_head<R: CommandRunner>(git: &Git<'_, R>, run: &Run, expected: Option<&Revision>) -> Result<String, MergeError> {
  let branch = run.branch();
  let head = git
    .commit(&format!("refs/heads/{branch}"))?
    .ok_or_else(|| MergeError::BranchMissing(branch.clone()))?;
  if let Some(expected) = expected
    && head != expected.as_str()
  {
    return Err(MergeError::BranchMoved {
      branch: branch.clone(),
      expected: expected.clone(),
    });
  }
  Ok(head)
}

/// The merge commit's title: *Merge ORC-12 run #3*.
fn title(run: &Run, task: &TaskKey) -> String {
  format!("Merge {task} run #{}", run.number().get())
}

/// The commit a merge that cannot fast-forward makes: as `delivering` says, else a merge commit titled `title`.
fn made<'a>(title: &'a str, delivering: Option<Delivering<'a>>) -> (temporary::Made<'a>, MergeKind) {
  match delivering.map(|delivering| delivering.as_delivered) {
    Some(DeliveredAs::Squash { title, body }) => (temporary::Made::Squash { title, body }, MergeKind::Squash),
    Some(DeliveredAs::MergeCommit) | None => (temporary::Made::Merge { title }, MergeKind::MergeCommit),
  }
}

/// Refuses a checkout that is not on `into` or has an operation in progress.
fn check_checkout<R: CommandRunner>(git: &Git<'_, R>, into: &BranchName) -> Result<(), MergeError> {
  let current = git.checked_out_branch()?;
  if current.as_ref() != Some(into) {
    return Err(MergeError::WrongBranch {
      expected: into.clone(),
      current,
    });
  }
  if in_progress(git)? {
    return Err(MergeError::InProgress);
  }
  Ok(())
}

/// Whether a merge, cherry-pick or revert is in progress where `git` runs.
pub(super) fn in_progress<R: CommandRunner>(git: &Git<'_, R>) -> Result<bool, GitError> {
  for operation in IN_PROGRESS {
    if git.succeeds(&["rev-parse", "--verify", "--quiet", operation])? {
      return Ok(true);
    }
  }
  Ok(false)
}

fn revision(sha: &str) -> Result<Revision, GitError> {
  Revision::new(sha).map_err(|_| GitError::Unrecognized {
    expected: "a commit",
    detail: sha.to_owned(),
  })
}
