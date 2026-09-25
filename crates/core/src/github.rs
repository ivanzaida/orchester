//! The GitHub provider boundary. Orchester reaches GitHub through a provider acting as the
//! user's signed-in account; the first one is the GitHub CLI, [`GhCli`].

use std::path::PathBuf;

use crate::{
  command::CommandRunner,
  connection::{GitHubCheck, GitHubConnection, GitHubHost, check_github},
  repository::{GitHubRepo, RepositoryError, RepositoryView, gh_repo_view_command, parse_gh_repo_view},
  time::Timestamp,
};

/// GitHub as one signed-in account on one host sees it.
pub trait GitHub {
  /// The provider program and the account it acts as.
  fn check(&self, now: Timestamp) -> GitHubCheck;

  /// `repo` as the account sees it. `repo` must be on the provider's host.
  fn repository(&self, repo: &GitHubRepo) -> Result<RepositoryView, RepositoryError>;
}

/// GitHub through the `gh` program of a GitHub connection, run with `runner`.
pub struct GhCli<R> {
  runner: R,
  executable: PathBuf,
  host: GitHubHost,
}

impl<R: CommandRunner> GhCli<R> {
  pub fn new(runner: R, connection: &GitHubConnection) -> Self {
    Self::with_program(runner, connection.executable().to_path_buf(), connection.host().clone())
  }

  /// `gh` at `executable` for `host`, before a connection to it exists (project setup).
  pub const fn with_program(runner: R, executable: PathBuf, host: GitHubHost) -> Self {
    Self {
      runner,
      executable,
      host,
    }
  }
}

impl<R: CommandRunner> GitHub for GhCli<R> {
  fn check(&self, now: Timestamp) -> GitHubCheck {
    check_github(&self.executable, &self.host, &self.runner, now)
  }

  fn repository(&self, repo: &GitHubRepo) -> Result<RepositoryView, RepositoryError> {
    if repo.host() != &self.host {
      return Err(RepositoryError::OtherHost {
        repo: repo.clone(),
        host: self.host.clone(),
      });
    }
    let output = self
      .runner
      .run(&gh_repo_view_command(&self.executable, repo))
      .map_err(|source| RepositoryError::Run { program: "gh", source })?;
    parse_gh_repo_view(repo, &output)
  }
}
