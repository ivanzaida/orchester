//! Removing a run's worktrees as a request carried out where the worktrees are (`docs/architecture/remote-runners.md`,
//! rule 5): the desktop records the removal in the project's database, before and after, and asks the project's runner
//! to remove ([`RemovalRequest`]), which [`serve`] carries out there. Every value here crosses the desktop's connection
//! to the runner as it is.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Cleanup, CleanupError, GitError, Leftovers, clean_up, clean_up_all, retry_held};
use crate::{
  command::{CommandError, CommandRunner},
  run::Run,
};

/// What removing a run's worktrees may lose.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Discard {
  /// Only the uncommitted changes a person saw, `confirmed` ([`clean_up`]), and no branch with commits merged nowhere.
  Seen(Option<Leftovers>),
  /// Everything the run left, unmerged work included: deleting its task ([`clean_up_all`]).
  Everything,
}

/// What the desktop asks of the worktrees of one run, which ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovalRequest {
  pub discard: Discard,
  /// A folder something held (on Windows a process of the run with a file open) is deleted again once the run's own
  /// processes running from it are stopped ([`retry_held`]): *Remove worktree* and a merge's clean-up do, deleting a
  /// task does not.
  pub retry_held: bool,
}

/// Why a [`RemovalRequest`] removed nothing: [`CleanupError`] as it crosses to the desktop, git's failure told as the
/// desktop words it.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RemovalFailure {
  #[error("the run still works in its worktrees")]
  LiveWork,
  #[error("local integration must be released before removing its worktrees")]
  LocalIntegration,
  /// The git program does not exist.
  #[error("git was not found")]
  GitMissing,
  /// Git could not be run, or printed what was not expected; `detail` says how.
  #[error("git stopped: {0}")]
  GitStopped(String),
  /// Git refused, with its message without credentials.
  #[error("git failed: {0}")]
  GitRefused(String),
  /// The run whose worktrees were to go could not be read where they are; `detail` says why.
  #[error("the run cannot be read: {0}")]
  RunUnreadable(String),
}

impl From<CleanupError> for RemovalFailure {
  fn from(error: CleanupError) -> Self {
    match error {
      CleanupError::LiveWork => Self::LiveWork,
      CleanupError::LocalIntegration => Self::LocalIntegration,
      CleanupError::Git(GitError::Run(CommandError::NotFound)) => Self::GitMissing,
      CleanupError::Git(error @ (GitError::Run(_) | GitError::Unrecognized { .. })) => {
        Self::GitStopped(error.to_string())
      }
      CleanupError::Git(GitError::Failed(said)) => Self::GitRefused(said),
    }
  }
}

impl RemovalFailure {
  /// The [`CleanupError`] the desktop tells a person about, as it told it when it removed worktrees itself; `None` for
  /// a run the runner could not read.
  pub fn cleanup_error(self) -> Option<CleanupError> {
    Some(match self {
      Self::LiveWork => CleanupError::LiveWork,
      Self::LocalIntegration => CleanupError::LocalIntegration,
      Self::GitMissing => CleanupError::Git(GitError::Run(CommandError::NotFound)),
      Self::GitStopped(detail) => CleanupError::Git(GitError::Run(CommandError::Failed(detail))),
      Self::GitRefused(said) => CleanupError::Git(GitError::Failed(said)),
      Self::RunUnreadable(_) => return None,
    })
  }
}

/// Carries out `request` for `run` in the repository at `repo` with the `git` program on `runner`: removes what the run
/// left as [`clean_up`] or [`clean_up_all`] does and, when asked, tries again the folders something held once `stop`
/// ended what of the run runs in each.
pub fn serve(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  run: &Run,
  request: &RemovalRequest,
  stop: impl FnMut(&Path),
) -> Result<Cleanup, RemovalFailure> {
  let mut cleanup = match &request.discard {
    Discard::Seen(confirmed) => clean_up(runner, git, repo, run, confirmed.as_ref()),
    Discard::Everything => clean_up_all(runner, git, repo, run),
  }?;
  if request.retry_held {
    retry_held(runner, (git, repo), &mut cleanup, stop);
  }
  Ok(cleanup)
}
