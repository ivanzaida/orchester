//! Detecting the project folder's GitHub repository (`docs/design/orchester-workspace.md`,
//! "GitHub", "Project setup detection"): the folder's git remote, the GitHub CLI of the remote's
//! host, and what the account `gh` acts as sees of the repository. Only read-only commands run:
//! `git remote -v`, `gh --version`, `gh auth status` and `gh repo view`.

use std::path::{Path, PathBuf};

use super::{
  RemoteName, RepositoryError, RepositoryLink, RepositoryView, read_remotes, remote::Remote, url::GitHubRepo,
};
use crate::{
  command::CommandRunner,
  connection::{GitHubCheck, GitHubCheckResult, GitHubHost, GitHubLogin, GitHubState},
  github::{GhCli, GitHub},
  id::ConnectionId,
  time::Timestamp,
};

/// A GitHub host Orchester knows about, with the `gh` program to ask about it, if one is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhProgram {
  pub host: GitHubHost,
  /// `None` when no `gh` was found for the host.
  pub executable: Option<PathBuf>,
}

/// What to detect: the repository at `repo_root`, through its remote `remote`.
#[derive(Clone, Debug)]
pub struct DetectRequest<'a> {
  pub repo_root: &'a Path,
  /// The `git` program to run.
  pub git: &'a Path,
  pub remote: &'a RemoteName,
  /// Hosts whose repositories count as GitHub repositories. A remote on any other host is not
  /// on GitHub.
  pub programs: &'a [GhProgram],
}

/// What detection found out about the folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Detection {
  /// git does not see a repository in the folder.
  NotAGitRepository,
  /// git could not be run or failed for another reason.
  GitFailed(RepositoryError),
  /// The repository has no remote of the requested name.
  NoRemote(RemoteName),
  /// The remote does not name a repository on a known GitHub host.
  NotGitHub(Remote),
  /// The remote names a repository on a GitHub host.
  GitHub(Box<GitHubDetection>),
}

/// A remote that names a GitHub repository, and what `gh` said about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubDetection {
  remote: Remote,
  repo: GitHubRepo,
  program: Option<PathBuf>,
  check: Option<GitHubCheck>,
  view: Option<Result<RepositoryView, RepositoryError>>,
}

/// Detection as the user reads it, for a project that expects `expected` (setup states
/// 01.C1–C10 and the detected block `XcbNT`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DetectionState {
  /// The repository is readable by the signed-in account, which has every scope needed.
  Detected {
    account: GitHubLogin,
  },
  NotAGitRepository,
  GitFailed,
  NoRemote,
  NotGitHub,
  /// No `gh` for the host, or nothing at its path.
  GhNotFound,
  NotSignedIn,
  WrongAccount {
    active: GitHubLogin,
    expected: GitHubLogin,
  },
  MissingScope {
    account: GitHubLogin,
  },
  /// GitHub found no repository the account can read.
  NoAccess {
    account: GitHubLogin,
  },
  /// `gh` answered in a way Orchester does not read.
  Unrecognized,
  /// `gh` could not be started or did not answer in time.
  CouldNotRun,
  Unreachable,
  RateLimited,
  /// The repository is empty: it has no default branch yet.
  NoDefaultBranch,
}

impl DetectionState {
  /// A project cannot be created in the folder: runs need git worktrees. GitHub states never
  /// block it.
  pub const fn blocks_project(&self) -> bool {
    matches!(self, Self::NotAGitRepository)
  }
}

/// Detects the GitHub repository of `request.repo_root` with `runner`, checking `gh` at `now`.
pub fn detect_repository(runner: &impl CommandRunner, request: &DetectRequest<'_>, now: Timestamp) -> Detection {
  let remotes = match read_remotes(runner, request.git, request.repo_root) {
    Ok(remotes) => remotes,
    Err(RepositoryError::NotAGitRepository) => return Detection::NotAGitRepository,
    Err(error) => return Detection::GitFailed(error),
  };
  let Some(remote) = remotes.into_iter().find(|remote| remote.name() == request.remote) else {
    return Detection::NoRemote(request.remote.clone());
  };
  let found = remote.github_repo().and_then(|repo| {
    let program = request.programs.iter().find(|program| &program.host == repo.host())?;
    Some((repo, program.executable.clone()))
  });
  let Some((repo, program)) = found else {
    return Detection::NotGitHub(remote);
  };
  let (check, view) = match &program {
    None => (None, None),
    Some(executable) => {
      let gh = GhCli::with_program(runner, executable.clone(), repo.host().clone());
      let check = gh.check(now);
      let view = check.result.signed_in_account().map(|_| gh.repository(&repo));
      (Some(check), view)
    }
  };
  Detection::GitHub(Box::new(GitHubDetection {
    remote,
    repo,
    program,
    check,
    view,
  }))
}

impl Detection {
  pub fn state(&self, expected: Option<&GitHubLogin>) -> DetectionState {
    match self {
      Self::NotAGitRepository => DetectionState::NotAGitRepository,
      Self::GitFailed(_) => DetectionState::GitFailed,
      Self::NoRemote(_) => DetectionState::NoRemote,
      Self::NotGitHub(_) => DetectionState::NotGitHub,
      Self::GitHub(found) => found.state(expected),
    }
  }

  /// The remote detection used, when the repository has it.
  pub fn remote(&self) -> Option<&Remote> {
    match self {
      Self::NotGitHub(remote) => Some(remote),
      Self::GitHub(found) => Some(&found.remote),
      Self::NotAGitRepository | Self::GitFailed(_) | Self::NoRemote(_) => None,
    }
  }

  pub fn github(&self) -> Option<&GitHubDetection> {
    match self {
      Self::GitHub(found) => Some(found),
      _ => None,
    }
  }

  /// The account could read the repository, so the project can link it.
  pub fn is_linkable(&self) -> bool {
    self
      .github()
      .is_some_and(|found| found.view().is_some() && found.account().is_some())
  }

  /// The link to record for a project that expects `expected`, or, when it expects nobody yet,
  /// the account `gh` acts as. `None` unless the account could read the repository.
  pub fn link(
    &self,
    connection: ConnectionId,
    expected: Option<&GitHubLogin>,
    now: Timestamp,
  ) -> Option<RepositoryLink> {
    let found = self.github()?;
    let view = found.view()?.clone();
    let expected = expected.or_else(|| found.account())?.clone();
    Some(RepositoryLink::new(
      connection,
      found.remote.name().clone(),
      view,
      expected,
      now,
    ))
  }
}

impl GitHubDetection {
  pub const fn remote(&self) -> &Remote {
    &self.remote
  }

  /// The repository as the remote names it.
  pub const fn repo(&self) -> &GitHubRepo {
    &self.repo
  }

  /// The `gh` program asked, if one was known for the host.
  pub fn program(&self) -> Option<&Path> {
    self.program.as_deref()
  }

  pub const fn check(&self) -> Option<&GitHubCheck> {
    self.check.as_ref()
  }

  /// The repository as the signed-in account sees it, when it could read it.
  pub fn view(&self) -> Option<&RepositoryView> {
    self.view.as_ref()?.as_ref().ok()
  }

  /// The account `gh` acts as on the host.
  pub fn account(&self) -> Option<&GitHubLogin> {
    self.check.as_ref()?.result.signed_in_account()
  }

  /// The `gh` version, when the program answered as the GitHub CLI.
  pub fn version(&self) -> Option<&str> {
    match &self.check.as_ref()?.result {
      GitHubCheckResult::Available { version, .. } => Some(version),
      _ => None,
    }
  }

  fn state(&self, expected: Option<&GitHubLogin>) -> DetectionState {
    let Some(check) = &self.check else {
      return DetectionState::GhNotFound;
    };
    let account = match GitHubState::of(Some(&check.result), expected) {
      GitHubState::SignedIn { account } => account,
      // Checked against an expected account, and the scope is what the user fixes first.
      GitHubState::WrongAccount { active, expected } => return DetectionState::WrongAccount { active, expected },
      GitHubState::MissingScope { account } => return DetectionState::MissingScope { account },
      GitHubState::NotSignedIn => return DetectionState::NotSignedIn,
      GitHubState::ProgramMissing => return DetectionState::GhNotFound,
      GitHubState::Unrecognized | GitHubState::SignInUnknown => return DetectionState::Unrecognized,
      GitHubState::NotChecked | GitHubState::CouldNotRun => return DetectionState::CouldNotRun,
    };
    match &self.view {
      Some(Ok(_)) => DetectionState::Detected { account },
      Some(Err(error)) => view_failure(error, account),
      // A signed-in account is always asked for the repository.
      None => DetectionState::Unrecognized,
    }
  }
}

fn view_failure(error: &RepositoryError, account: GitHubLogin) -> DetectionState {
  match error {
    RepositoryError::NotFound(_) | RepositoryError::NoAccess(_) => DetectionState::NoAccess { account },
    RepositoryError::NotSignedIn(_) => DetectionState::NotSignedIn,
    RepositoryError::RateLimited => DetectionState::RateLimited,
    RepositoryError::Network { .. } => DetectionState::Unreachable,
    RepositoryError::NoDefaultBranch(_) => DetectionState::NoDefaultBranch,
    RepositoryError::Run { .. } => DetectionState::CouldNotRun,
    RepositoryError::NotAGitRepository
    | RepositoryError::Git(_)
    | RepositoryError::NoGitHubRemote(_)
    | RepositoryError::NonGitHubRemote { .. }
    | RepositoryError::OtherHost { .. }
    | RepositoryError::GitHubUnavailable(_)
    | RepositoryError::AccountMismatch { .. }
    | RepositoryError::Unrecognized { .. } => DetectionState::Unrecognized,
  }
}
