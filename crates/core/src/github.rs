//! The GitHub provider boundary. Orchester reaches GitHub through a provider acting as the
//! user's signed-in account; the first one is the GitHub CLI, [`GhCli`].

use std::path::PathBuf;

use crate::{
  command::{Command, CommandOutput, CommandRunner},
  connection::{GitHubCheck, GitHubConnection, GitHubHost, check_github},
  pull_request::{
    Fields, ListedStates, PullRequest, PullRequestHead, gh_check_rollups_command, gh_compare_diff_command,
    gh_pr_list_command, gh_pr_view_command, gh_rate_limit_command, parse_gh_check_rollups, parse_gh_compare_diff,
    parse_gh_pr_list, parse_gh_pr_view, parse_gh_rate_limit,
  },
  repository::{GitHubRepo, RepositoryError, RepositoryView, gh_repo_view_command, parse_gh_repo_view, unrecognized},
  revision::Revision,
  time::Timestamp,
};

/// GitHub as one signed-in account on one host sees it.
pub trait GitHub {
  /// The provider program and the account it acts as.
  fn check(&self, now: Timestamp) -> GitHubCheck;

  /// `repo` as the account sees it. `repo` must be on the provider's host.
  fn repository(&self, repo: &GitHubRepo) -> Result<RepositoryView, RepositoryError>;

  /// At most `limit` pull requests of `repo` in `states`, read with `fields`; `repo` must be on the
  /// provider's host. Each comes with the checks of its head.
  fn list_pull_requests(
    &self,
    repo: &GitHubRepo,
    query: (ListedStates, Fields),
    limit: u32,
  ) -> Result<Vec<PullRequest>, RepositoryError>;

  /// When the rate limit that stopped a request about `repo` resets.
  fn rate_limit_reset(&self, repo: &GitHubRepo) -> Result<Timestamp, RepositoryError>;

  /// Pull request `number` of `repo` now, read with `fields`, with its base revision.
  fn pull_request(&self, repo: &GitHubRepo, number: u64, fields: Fields) -> Result<PullRequestHead, RepositoryError>;

  /// The unified diff of `repo` from the merge base of `base` and `head` to `head`.
  fn compare_diff(&self, repo: &GitHubRepo, base: &Revision, head: &Revision) -> Result<String, RepositoryError>;
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
    let output = self.run_for(repo, &gh_repo_view_command(&self.executable, repo))?;
    parse_gh_repo_view(repo, &output)
  }

  fn list_pull_requests(
    &self,
    repo: &GitHubRepo,
    (states, fields): (ListedStates, Fields),
    limit: u32,
  ) -> Result<Vec<PullRequest>, RepositoryError> {
    let output = self.run_for(
      repo,
      &gh_pr_list_command(&self.executable, repo, (states, fields), limit),
    )?;
    let listed = parse_gh_pr_list(repo, &output)?;
    if listed.is_empty() {
      return Ok(listed);
    }
    let heads: Vec<Revision> = listed
      .iter()
      .map(|pull_request| pull_request.head_sha().clone())
      .collect();
    let output = self.run_for(repo, &gh_check_rollups_command(&self.executable, repo, &heads))?;
    let checks = parse_gh_check_rollups(repo, &heads, &output)?;
    listed
      .into_iter()
      .zip(checks)
      .map(|(pull_request, checks)| {
        pull_request
          .with_checks(checks)
          .map_err(|error| unrecognized(&error.to_string()))
      })
      .collect()
  }

  fn rate_limit_reset(&self, repo: &GitHubRepo) -> Result<Timestamp, RepositoryError> {
    let output = self.run_for(repo, &gh_rate_limit_command(&self.executable, repo.host()))?;
    parse_gh_rate_limit(repo, &output)
  }

  fn pull_request(&self, repo: &GitHubRepo, number: u64, fields: Fields) -> Result<PullRequestHead, RepositoryError> {
    let output = self.run_for(repo, &gh_pr_view_command(&self.executable, repo, number, fields))?;
    parse_gh_pr_view(repo, number, &output)
  }

  fn compare_diff(&self, repo: &GitHubRepo, base: &Revision, head: &Revision) -> Result<String, RepositoryError> {
    let output = self.run_for(repo, &gh_compare_diff_command(&self.executable, repo, base, head))?;
    parse_gh_compare_diff(repo, &output)
  }
}

impl<R: CommandRunner> GhCli<R> {
  /// Runs `command` about `repo`, which must be on the provider's host.
  fn run_for(&self, repo: &GitHubRepo, command: &Command) -> Result<CommandOutput, RepositoryError> {
    if repo.host() != &self.host {
      return Err(RepositoryError::OtherHost {
        repo: repo.clone(),
        host: self.host.clone(),
      });
    }
    self
      .runner
      .run(command)
      .map_err(|source| RepositoryError::Run { program: "gh", source })
  }
}
