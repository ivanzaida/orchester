//! Remotes of the local repository, read with `git remote -v` (read-only).

use std::path::Path;

use super::{
  RemoteName, RepositoryError,
  url::{GitHubRepo, redact_userinfo, strip_userinfo},
};
use crate::command::{Command, CommandOutput, CommandRunner};

const GIT: &str = "git";
const FETCH_SUFFIX: &str = " (fetch)";

/// A git remote and the URL it fetches from, without the URL's user part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
  name: RemoteName,
  url: String,
}

impl Remote {
  pub const fn name(&self) -> &RemoteName {
    &self.name
  }

  pub fn url(&self) -> &str {
    &self.url
  }

  /// The GitHub repository the remote points at, if its URL names one.
  pub fn github_repo(&self) -> Option<GitHubRepo> {
    GitHubRepo::from_remote_url(&self.url)
  }
}

/// `git remote -v` in `repo_root`. Git's messages are read in English, so its translations are
/// turned off.
pub fn git_remotes_command(git: &Path, repo_root: &Path) -> Command {
  Command::new(git, ["remote", "-v"])
    .with_current_dir(repo_root)
    .with_env("LC_ALL", "C")
}

/// The fetch remotes in `git remote -v` output (`origin\thttps://… (fetch)`), in output order.
/// Lines that are not remotes are skipped.
pub fn parse_git_remotes(stdout: &str) -> Vec<Remote> {
  stdout
    .lines()
    .filter_map(|line| {
      let (name, target) = line.split_once('\t')?;
      let url = target.trim_end().strip_suffix(FETCH_SUFFIX)?;
      Some(Remote {
        name: RemoteName::new(name).ok()?,
        url: strip_userinfo(url),
      })
    })
    .collect()
}

/// Reads the remotes of the repository at `repo_root` with the `git` program at `git`.
pub fn read_remotes(runner: &impl CommandRunner, git: &Path, repo_root: &Path) -> Result<Vec<Remote>, RepositoryError> {
  let output = runner
    .run(&git_remotes_command(git, repo_root))
    .map_err(|source| RepositoryError::Run { program: GIT, source })?;
  if output.success {
    return Ok(parse_git_remotes(&output.stdout));
  }
  Err(git_failure(&output))
}

fn git_failure(output: &CommandOutput) -> RepositoryError {
  let stderr = redact_userinfo(output.stderr.trim());
  if stderr.contains("not a git repository") {
    RepositoryError::NotAGitRepository
  } else {
    RepositoryError::Git(stderr)
  }
}
