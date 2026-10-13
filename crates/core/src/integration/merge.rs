//! Merging a run that succeeded into the branch the user's checkout had when it started, in that
//! checkout. It merges only a clean checkout on that branch with no merge, cherry-pick or revert
//! in progress, and never stashes, resets or touches uncommitted work: anything else is refused
//! with the reason. It fast-forwards when it can, else commits a merge ("Merge ORC-12 run #3")
//! with the repository's git identity. A conflict, or a hook that refuses the merge commit, is
//! undone with `git merge --abort`, which leaves the clean checkout as it was. The repository's
//! hooks run: they are the user's.

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

/// Why a run was not merged. Except for [`MergeError::Git`] after the merge started, nothing in
/// the checkout changed.
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
  #[error("the checkout has uncommitted changes: {files:?}")]
  Uncommitted { files: Vec<String> },
  #[error("a merge, cherry-pick or revert is in progress in the checkout")]
  InProgress,
  #[error("the run's branch {0} does not exist")]
  BranchMissing(BranchName),
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
  let (revision, kind) = merge(runner, git, repo, run, task)?;
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
  git: &Path,
  repo: &Path,
  run: &Run,
  task: &TaskKey,
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
  let base = git
    .commit("HEAD")?
    .ok_or_else(|| GitError::Failed("HEAD names no commit".to_owned()))?;
  if git.is_ancestor(&head, &base)? {
    return Ok((revision(&base)?, MergeKind::AlreadyMerged));
  }
  if git.is_ancestor(&base, &head)? {
    let output = git.output(&["merge", "--ff-only", "--no-edit", "--no-overwrite-ignore", &head])?;
    if !output.success() {
      return Err(MergeError::Refused(message(&output)));
    }
    return Ok((revision(&head)?, MergeKind::FastForward));
  }
  let title = format!("Merge {task} run #{}", run.number().get());
  let output = git.output(&[
    "merge",
    "--no-ff",
    "--no-edit",
    "--no-overwrite-ignore",
    "-m",
    &title,
    &head,
  ])?;
  if !output.success() {
    return Err(undo(&git, message(&output)));
  }
  let merged = git.stdout(&["rev-parse", "--verify", "HEAD"])?;
  Ok((revision(&merged)?, MergeKind::MergeCommit))
}

/// Refuses a checkout that is not on `into`, has uncommitted changes or an operation in progress.
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
  let files = git.uncommitted()?;
  if !files.is_empty() {
    return Err(MergeError::Uncommitted { files });
  }
  Ok(())
}

/// Undoes a merge that stopped, naming the files that conflict. A merge refused before it began
/// left nothing to undo.
fn undo<R: CommandRunner>(git: &Git<'_, R>, said: String) -> MergeError {
  let started = match git.succeeds(&["rev-parse", "--verify", "--quiet", "MERGE_HEAD"]) {
    Ok(started) => started,
    Err(error) => return error.into(),
  };
  if !started {
    return MergeError::Refused(said);
  }
  let conflicts = git.stdout(&["diff", "--name-only", "--diff-filter=U", "-z"]);
  if let Err(error) = git.stdout(&["merge", "--abort"]) {
    return error.into();
  }
  match conflicts {
    Ok(files) if !files.is_empty() => MergeError::Conflict {
      files: files
        .split('\0')
        .filter(|file| !file.is_empty())
        .map(str::to_owned)
        .collect(),
    },
    Ok(_) => MergeError::Refused(said),
    Err(error) => error.into(),
  }
}

fn revision(sha: &str) -> Result<Revision, GitError> {
  Revision::new(sha).map_err(|_| GitError::Unrecognized {
    expected: "a commit",
    detail: sha.to_owned(),
  })
}
