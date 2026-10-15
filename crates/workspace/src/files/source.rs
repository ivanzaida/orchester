//! Where a version's files are: a folder (the project's checkout or a run's worktree) or a commit of the
//! repository, and the read-only git commands that read a commit.

use std::path::{Path, PathBuf};

use orchester_core::{
  command::{CommandError, CommandRunner},
  diff::git::git_command,
  integration::{GitError, checked_out_branch},
  repository::redact_userinfo,
  run::BranchName,
};

use super::{FilesError, Version};
use crate::tasks::changes::{GIT, run_worktree};

/// Git's option that takes the paths it is given as paths, never as patterns.
pub(super) const LITERAL_PATHS: &str = "--literal-pathspecs";

/// Where the files of a version are read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Source {
  Folder(PathBuf),
  /// A commit of the repository at `repo`, named as git takes it; `branch` when it is a branch's tip.
  Commit {
    repo: PathBuf,
    commit: String,
    branch: Option<BranchName>,
  },
}

/// Where `version` of the project at `repo_root` is, or [`FilesError::VersionGone`] when it no longer
/// exists: a removed worktree, a commit the repository lost, a checkout without a branch.
pub(super) fn source(runner: &dyn CommandRunner, repo_root: &Path, version: &Version) -> Result<Source, FilesError> {
  let gone = || FilesError::VersionGone(version.clone());
  match version {
    Version::Checkout => Ok(Source::Folder(repo_root.to_path_buf())),
    Version::Run(run) => {
      let worktree = run_worktree(repo_root, *run);
      if worktree.is_dir() {
        Ok(Source::Folder(worktree))
      } else {
        Err(gone())
      }
    }
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
