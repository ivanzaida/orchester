//! Merging a run that succeeded into the branch the user's checkout had when it started, in that
//! checkout. It merges only a checkout on that branch with no merge, cherry-pick or revert in
//! progress, and never stashes, resets or touches uncommitted work: uncommitted changes and
//! untracked files the merge does not reach stay as they are. It fast-forwards when it can, else
//! commits a merge ("Merge ORC-12 run #3") with the repository's git identity in a temporary
//! worktree ([`temporary`]), where a conflict or a hook that refuses the merge commit stops it
//! without reaching the checkout. Either result reaches the checkout as a fast-forward
//! ([`bring_in`]), which git refuses, changing nothing, when it would overwrite or remove a file
//! with uncommitted changes, an untracked one or an ignored one; the refusal names exactly those.
//! The repository's hooks run: they are the user's.

mod temporary;

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
  GitError,
  git::{Git, message},
};
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

/// Why a run was not merged. Except for [`MergeError::Git`] while the checkout was being brought
/// to the merge, nothing in the checkout changed.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MergeError {
  #[error("only a run that succeeded can be merged")]
  NotSucceeded,
  /// The run started on a detached HEAD, or before Orchester recorded the branch.
  #[error("the run did not record the branch it started from")]
  NoBaseBranch,
  /// `current` is `None` on a detached HEAD.
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
  /// The run's branch is no longer at the commit the merge was asked for, e.g. the one a person reviewed.
  #[error("the run's branch {branch} is no longer at {expected}")]
  BranchMoved { branch: BranchName, expected: Revision },
  /// Both sides changed these files; the merge was undone.
  #[error("the run's branch conflicts with the base branch in {files:?}")]
  Conflict { files: Vec<String> },
  /// Git did not merge, e.g. a hook refused the merge commit; the merge was undone. Its message.
  #[error("git did not merge: {0}")]
  Refused(String),
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
) -> Result<RunMerge, MergeError> {
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
) -> Result<RunMerge, MergeError> {
  let (revision, kind) = merge(runner, (git, repo), (run, task), expected)?;
  Ok(RunMerge {
    run: run.id(),
    into: run.base_branch().ok_or(MergeError::NoBaseBranch)?.clone(),
    revision,
    kind,
    merged_at: now,
  })
}

fn merge(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  (run, task): (&Run, &TaskKey),
  expected: Option<&Revision>,
) -> Result<(Revision, MergeKind), MergeError> {
  if run.status() != &RunStatus::Succeeded {
    return Err(MergeError::NotSucceeded);
  }
  let into = run.base_branch().ok_or(MergeError::NoBaseBranch)?;
  let git = Git::new(runner, git, repo);
  check_checkout(&git, into)?;
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
  let base = git
    .commit("HEAD")?
    .ok_or_else(|| GitError::Failed("HEAD names no commit".to_owned()))?;
  if git.is_ancestor(&head, &base)? {
    return Ok((revision(&base)?, MergeKind::AlreadyMerged));
  }
  if git.is_ancestor(&base, &head)? {
    bring_in(&git, &head)?;
    return Ok((revision(&head)?, MergeKind::FastForward));
  }
  let title = format!("Merge {task} run #{}", run.number().get());
  let merged = temporary::merge_commit(&git, run.id(), (&base, &head), &title)?;
  bring_in(&git, &merged)?;
  Ok((revision(&merged)?, MergeKind::MergeCommit))
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
  for operation in IN_PROGRESS {
    if git.succeeds(&["rev-parse", "--verify", "--quiet", operation])? {
      return Err(MergeError::InProgress);
    }
  }
  Ok(())
}

/// Fast-forwards the checkout's branch, index and files to `commit`, a descendant of its HEAD. Git
/// checks every path before it writes one, so a refusal changed nothing.
fn bring_in<R: CommandRunner>(git: &Git<'_, R>, commit: &str) -> Result<(), MergeError> {
  let output = git.output(&["merge", "--ff-only", "--no-edit", "--no-overwrite-ignore", commit])?;
  if output.success() {
    return Ok(());
  }
  let files = in_the_way(&output.stderr);
  if files.is_empty() {
    return Err(MergeError::Refused(message(&output)));
  }
  Err(MergeError::UncommittedInTheWay { files })
}

/// The files git names when it refuses to update the checkout because that would lose what is in
/// them: one per line after a tab, under a line that ends like "would be overwritten by merge:",
/// "would be removed by merge:" or "would lose untracked files in them:" (git's messages are in
/// English, [`crate::diff::git::git_command`]).
fn in_the_way(stderr: &str) -> Vec<String> {
  let mut files = Vec::new();
  let mut listing = false;
  for line in stderr.lines() {
    match line.strip_prefix('\t') {
      Some(file) if listing => files.push(file.to_owned()),
      Some(_) => {}
      None => listing = line.ends_with(" by merge:") || line.ends_with(" would lose untracked files in them:"),
    }
  }
  files
}

fn revision(sha: &str) -> Result<Revision, GitError> {
  Revision::new(sha).map_err(|_| GitError::Unrecognized {
    expected: "a commit",
    detail: sha.to_owned(),
  })
}
