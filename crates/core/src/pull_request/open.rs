//! Opening a pull request for a run's branch on the project's linked repository (`docs/design/
//! orchester-workspace.md`, "GitHub", rules 4 to 7): `gh` must act as the account the link expects, with the
//! `repo` scope; then the branch is pushed with git's own credentials, and `gh api` asks GitHub to open the
//! pull request. Orchester never signs in, switches accounts or changes git config on the way.

use std::path::Path;

use super::{
  create::{CreateFailure, NewPullRequest, OpenedPullRequest},
  push::{PushFailure, git_push_command, parse_git_push},
};
use crate::{
  command::CommandRunner,
  connection::{GitHubCheck, GitHubState},
  github::GitHub,
  repository::RepositoryLink,
  time::Timestamp,
};

/// Where the branch is pushed from: git run by `runner`, the program `git`, in the repository at `repo_root`.
#[derive(Clone, Copy)]
pub struct Push<'a> {
  pub runner: &'a dyn CommandRunner,
  pub git: &'a Path,
  pub repo_root: &'a Path,
}

/// Why no pull request was opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenFailure {
  /// `gh` cannot act as the expected account with the `repo` scope; the state says why. Nothing was pushed.
  Account(GitHubState),
  /// The branch was not pushed, so nothing was asked of GitHub.
  Push(PushFailure),
  /// The branch was pushed, but GitHub opened no pull request.
  Create(CreateFailure),
}

/// The check of `gh` the attempt made, and what it came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenAttempt {
  pub check: GitHubCheck,
  pub outcome: Result<OpenedPullRequest, OpenFailure>,
}

/// Opens `new` on `link`'s repository through `github` once `push` pushed its head to `link`'s remote, if `gh`
/// acts as the account the link expects.
pub fn open_pull_request(
  github: &impl GitHub,
  push: Push<'_>,
  (link, new): (&RepositoryLink, &NewPullRequest),
  now: Timestamp,
) -> OpenAttempt {
  let check = github.check(now);
  let state = GitHubState::of(Some(&check.result), Some(link.expected_login()));
  if !matches!(state, GitHubState::SignedIn { .. }) {
    return OpenAttempt {
      check,
      outcome: Err(OpenFailure::Account(state)),
    };
  }
  let pushed = push
    .runner
    .run(&git_push_command(push.git, push.repo_root, link.remote(), &new.head))
    .map_err(PushFailure::NotRun)
    .and_then(|output| parse_git_push(&output));
  let outcome = match pushed {
    Err(failure) => Err(OpenFailure::Push(failure)),
    Ok(()) => github
      .create_pull_request(link.repo(), new)
      .map_err(OpenFailure::Create),
  };
  OpenAttempt { check, outcome }
}
