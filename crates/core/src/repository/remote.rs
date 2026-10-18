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

/// `git --git-dir <git_dir> remote -v`, run outside the checkout: the remotes read from the repository's metadata
/// alone, while another operation may hold the checkout (a local integration's review of it).
pub fn git_dir_remotes_command(git: &Path, git_dir: &Path) -> Command {
  let git_dir = git_dir.to_string_lossy().into_owned();
  Command::new(
    git,
    ["--git-dir".to_owned(), git_dir, "remote".to_owned(), "-v".to_owned()],
  )
  .with_env("LC_ALL", "C")
}

/// Reads the remotes of the repository at `repo_root` with the `git` program at `git`.
pub fn read_remotes(runner: &impl CommandRunner, git: &Path, repo_root: &Path) -> Result<Vec<Remote>, RepositoryError> {
  remotes(runner, &git_remotes_command(git, repo_root))
}

/// Reads the remotes of the repository whose metadata is `git_dir` ([`git_dir_remotes_command`]).
pub fn read_git_dir_remotes(
  runner: &impl CommandRunner,
  git: &Path,
  git_dir: &Path,
) -> Result<Vec<Remote>, RepositoryError> {
  remotes(runner, &git_dir_remotes_command(git, git_dir))
}

fn remotes(runner: &impl CommandRunner, command: &Command) -> Result<Vec<Remote>, RepositoryError> {
  let output = runner
    .run(command)
    .map_err(|source| RepositoryError::Run { program: GIT, source })?;
  if output.success() {
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
