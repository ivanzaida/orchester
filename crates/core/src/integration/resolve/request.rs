//! Resolving by hand as requests about a run's worktree, carried out where the worktree is
//! (`docs/architecture/remote-runners.md`, rule 5): the desktop keeps the project's record of the resolution and asks
//! the run's runner for each git command in the worktree ([`ResolveRequest`]), which [`serve`] carries out there.
//! Every value here crosses the desktop's connection to the runner as it is.

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{MergedIn, Resolution, ResolveError, Side, Started, holds_markers};
use crate::{
  command::{CommandError, CommandRunner},
  integration::{GitError, delivery},
  revision::Revision,
  run::BranchName,
};

/// What the desktop asks of a run's worktree while a person resolves a merge there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ResolveRequest {
  /// Whether its folder is there and, when it is, the merge in progress in it with how each conflicted file stands
  /// ([`super::resolution`]). Answered with [`ResolveAnswer::Read`].
  Read,
  /// Whether its folder is there. Answered with [`ResolveAnswer::Present`].
  Present,
  /// Whether a merge is in progress in it ([`super::in_merge`]). Answered with [`ResolveAnswer::InMerge`].
  InMerge,
  /// The commit its `HEAD` is at ([`delivery::head`]). Answered with [`ResolveAnswer::Head`].
  Head,
  /// Merges `base` into `branch`, checked out there at `expected` ([`super::start`]). Answered with
  /// [`ResolveAnswer::Started`].
  Start {
    base: BranchName,
    branch: BranchName,
    expected: Revision,
  },
  /// Keeps `side`'s version of `path` ([`super::decide`]). Answered with [`ResolveAnswer::Done`].
  Decide { path: String, side: Side },
  /// Commits the merge in progress of `base`, the branch still at `expected` ([`super::finish`]). Answered with
  /// [`ResolveAnswer::Finished`].
  Finish { base: BranchName, expected: Revision },
  /// Aborts the merge in progress ([`super::abort`]). Answered with [`ResolveAnswer::Done`].
  Abort,
  /// Which of `files` still hold a conflict marker, read as text; a file that cannot be read holds none. Answered with
  /// [`ResolveAnswer::Marked`].
  Marked { files: Vec<String> },
}

/// What a [`ResolveRequest`] found or did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ResolveAnswer {
  Read { present: bool, merge: Option<Resolution> },
  Present(bool),
  InMerge(bool),
  Head(Revision),
  Started(Started),
  Finished(MergedIn),
  Done,
  Marked(Vec<String>),
}

/// Why a [`ResolveRequest`] did not go on: [`ResolveError`] as it crosses to the desktop, with its own words.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[error("{detail}")]
pub struct ResolveFailure {
  pub reason: FailureReason,
  /// What [`ResolveError`] says of itself.
  pub detail: String,
}

/// The [`ResolveError`] behind a [`ResolveFailure`], with git's failure told as the desktop words it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum FailureReason {
  /// The git program does not exist.
  GitMissing,
  /// Git could not be run, or printed what was not expected.
  GitStopped,
  /// Git refused, with its message without credentials.
  GitRefused(String),
  Uncommitted(Vec<String>),
  NotOnBranch,
  BranchMoved,
  NotTheBase,
  InMerge,
  NotInMerge,
  MarkersLeft(Vec<String>),
  Undecided(Vec<String>),
  NotUnmerged(String),
}

impl From<ResolveError> for ResolveFailure {
  fn from(error: ResolveError) -> Self {
    let detail = error.to_string();
    let reason = match error {
      ResolveError::Git(GitError::Run(CommandError::NotFound)) => FailureReason::GitMissing,
      ResolveError::Git(GitError::Run(_) | GitError::Unrecognized { .. }) => FailureReason::GitStopped,
      ResolveError::Git(GitError::Failed(said)) => FailureReason::GitRefused(said),
      ResolveError::Uncommitted(files) => FailureReason::Uncommitted(files),
      ResolveError::NotOnBranch => FailureReason::NotOnBranch,
      ResolveError::BranchMoved => FailureReason::BranchMoved,
      ResolveError::NotTheBase => FailureReason::NotTheBase,
      ResolveError::InMerge => FailureReason::InMerge,
      ResolveError::NotInMerge => FailureReason::NotInMerge,
      ResolveError::MarkersLeft(files) => FailureReason::MarkersLeft(files),
      ResolveError::Undecided(files) => FailureReason::Undecided(files),
      ResolveError::NotUnmerged(path) => FailureReason::NotUnmerged(path),
    };
    Self { reason, detail }
  }
}

impl From<GitError> for ResolveFailure {
  fn from(error: GitError) -> Self {
    ResolveError::from(error).into()
  }
}

/// Carries out `request` in `worktree` with the `git` program: what reads it runs on `reads`, what changes it (and
/// whether a merge is in progress before a change) on `work`, as the desktop ran them before they moved here.
pub fn serve<R: CommandRunner, W: CommandRunner>(
  (reads, work): (&R, &W),
  git: &Path,
  worktree: &Path,
  request: &ResolveRequest,
) -> Result<ResolveAnswer, ResolveFailure> {
  Ok(match request {
    ResolveRequest::Read => {
      if !worktree.is_dir() {
        return Ok(ResolveAnswer::Read {
          present: false,
          merge: None,
        });
      }
      ResolveAnswer::Read {
        present: true,
        merge: super::resolution(reads, git, worktree)?,
      }
    }
    ResolveRequest::Present => ResolveAnswer::Present(worktree.is_dir()),
    ResolveRequest::InMerge => ResolveAnswer::InMerge(super::in_merge(work, git, worktree)?),
    ResolveRequest::Head => ResolveAnswer::Head(delivery::head(reads, git, worktree)?),
    ResolveRequest::Start { base, branch, expected } => {
      ResolveAnswer::Started(super::start(work, git, worktree, (base, branch, expected))?)
    }
    ResolveRequest::Decide { path, side } => {
      super::decide(work, git, worktree, (path, *side))?;
      ResolveAnswer::Done
    }
    ResolveRequest::Finish { base, expected } => {
      ResolveAnswer::Finished(super::finish(work, git, worktree, (base, expected))?)
    }
    ResolveRequest::Abort => {
      super::abort(work, git, worktree)?;
      ResolveAnswer::Done
    }
    ResolveRequest::Marked { files } => ResolveAnswer::Marked(marked(worktree, files)),
  })
}

/// The `files` of `worktree` whose text holds a conflict marker.
fn marked(worktree: &Path, files: &[String]) -> Vec<String> {
  files
    .iter()
    .filter(|file| fs::read_to_string(worktree.join(file.as_str())).is_ok_and(|text| holds_markers(&text)))
    .cloned()
    .collect()
}
