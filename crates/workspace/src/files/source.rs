//! Where a version's files are: the project's checkout, a run's worktree (which the project's runner reads,
//! `super::runs`) or a commit of the repository, and the read-only git commands that read a commit.

use std::path::{Path, PathBuf};

use orchester_core::{
  command::{CommandError, CommandRunner},
  diff::git::git_command,
  id::RunId,
  integration::{GitError, checked_out_branch},
  repository::redact_userinfo,
  run::BranchName,
};

use super::{FilesError, Version};
use crate::tasks::changes::GIT;

/// Git's option that takes the paths it is given as paths, never as patterns.
pub(super) const LITERAL_PATHS: &str = "--literal-pathspecs";

/// Where the files of a version are read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Source {
  /// The project's checkout.
  Folder(PathBuf),
  /// The worktree of a run: its runner reads it, and tells whether it is still there.
  Run(RunId),
  /// A commit of the repository at `repo`, named as git takes it; `branch` when it is a branch's tip.
  Commit {
    repo: PathBuf,
    commit: String,
    branch: Option<BranchName>,
  },
}

/// Where `version` of the project at `repo_root` is, or [`FilesError::VersionGone`] when it no longer
/// exists: a commit the repository lost, a checkout without a branch. Whether a run's worktree is still there
/// only its runner tells.
pub(super) fn source(runner: &dyn CommandRunner, repo_root: &Path, version: &Version) -> Result<Source, FilesError> {
  let gone = || FilesError::VersionGone(version.clone());
  match version {
    Version::Checkout => Ok(Source::Folder(repo_root.to_path_buf())),
    Version::Run(run) => Ok(Source::Run(*run)),
    Version::Revision(revision) => {
      let commit = format!("{revision}^{{commit}}");
      if git_succeeds(runner, repo_root, &["cat-file", "-e", &commit])? {
        Ok(Source::Commit {
          repo: repo_root.to_path_buf(),
          commit,
          branch: None,
        })
      } else {
        Err(gone())
      }
    }
    Version::Base => match checked_out_branch(&runner, Path::new(GIT), repo_root) {
      Ok(Some(branch)) => Ok(Source::Commit {
        repo: repo_root.to_path_buf(),
        commit: format!("refs/heads/{branch}"),
        branch: Some(branch),
      }),
      Ok(None) => Err(gone()),
      Err(error) => Err(git_error(error)),
    },
  }
}

/// `error` of a git read: git not found, or what git said.
pub(super) fn git_error(error: GitError) -> FilesError {
  match error {
    GitError::Run(error) => FilesError::from(error),
    error => FilesError::Git(error.to_string()),
  }
}

/// The standard output of `git <args>` in `directory`, which must succeed.
pub(super) fn git_output(runner: &dyn CommandRunner, directory: &Path, args: &[&str]) -> Result<String, FilesError> {
  let output = runner.run(&git_command(Path::new(GIT), directory, args.iter().copied()))?;
  if output.success() {
    Ok(output.stdout)
  } else {
    Err(FilesError::Git(redact_userinfo(output.stderr.trim())))
  }
}

/// Whether `git <args>` in `directory` succeeds: git's answer to a yes-or-no question.
pub(super) fn git_succeeds(runner: &dyn CommandRunner, directory: &Path, args: &[&str]) -> Result<bool, FilesError> {
  Ok(
    runner
      .run(&git_command(Path::new(GIT), directory, args.iter().copied()))?
      .success(),
  )
}

impl From<CommandError> for FilesError {
  fn from(error: CommandError) -> Self {
    match error {
      CommandError::NotFound => Self::GitMissing,
      other => Self::Git(other.to_string()),
    }
  }
}
