//! What the last check of a GitHub connection means for a project, and the `gh` commands the
//! user can run in a terminal to fix it (`docs/design/orchester-workspace.md`, "GitHub", rule 6).
//! Orchester never runs these commands: they are shown so the user can copy them.

use super::{GitHubCheckResult, GitHubHost, GitHubLogin};
use crate::connection::SignIn;

/// The only scope Orchester needs: reading repositories and creating pull requests.
pub const REQUIRED_SCOPE: &str = "repo";

/// Stands for an account the user picks in [`GhAdvice::Switch`] without an expected one.
const ANY_LOGIN: &str = "<login>";

/// A GitHub connection as the project sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitHubState {
  NotChecked,
  /// `gh` is signed in with the scope Orchester needs, as the expected account if there is one.
  SignedIn {
    account: GitHubLogin,
  },
  NotSignedIn,
  /// `gh` answered, but whether it is signed in could not be read.
  SignInUnknown,
  /// `gh` acts as `active`, but the project expects `expected`.
  WrongAccount {
    active: GitHubLogin,
    expected: GitHubLogin,
  },
  /// The sign-in has scopes, but not [`REQUIRED_SCOPE`].
  MissingScope {
    account: GitHubLogin,
  },
  ProgramMissing,
  /// The program did not answer as the GitHub CLI.
  Unrecognized,
  CouldNotRun,
}

impl GitHubState {
  /// The state after `check`, for a project that expects `expected` (the login of its
  /// repository link, if it has one).
  pub fn of(check: Option<&GitHubCheckResult>, expected: Option<&GitHubLogin>) -> Self {
    let Some(check) = check else {
      return Self::NotChecked;
    };
    let (account, scopes, sign_in) = match check {
      GitHubCheckResult::ExecutableMissing => return Self::ProgramMissing,
      GitHubCheckResult::NotRecognized => return Self::Unrecognized,
      GitHubCheckResult::CouldNotRun => return Self::CouldNotRun,
      GitHubCheckResult::Available {
        account,
        scopes,
        sign_in,
        ..
      } => (account, scopes, *sign_in),
    };
    match (sign_in, account) {
      (SignIn::SignedOut, _) => Self::NotSignedIn,
      (SignIn::Unknown, _) | (SignIn::SignedIn, None) => Self::SignInUnknown,
      (SignIn::SignedIn, Some(active)) => match expected {
        Some(expected) if expected != active => Self::WrongAccount {
          active: active.clone(),
          expected: expected.clone(),
        },
        // Tokens without OAuth scopes (fine-grained or app tokens) list none; only a list
        // without `repo` shows that it is missing.
        _ if !scopes.is_empty() && !scopes.iter().any(|scope| scope == REQUIRED_SCOPE) => Self::MissingScope {
          account: active.clone(),
        },
        _ => Self::SignedIn {
          account: active.clone(),
        },
      },
    }
  }

  /// The command that fixes this state, if a `gh` command does.
  pub fn advice(&self) -> Option<GhAdvice> {
    match self {
      Self::NotSignedIn => Some(GhAdvice::Login),
      Self::WrongAccount { expected, .. } => Some(GhAdvice::Switch(Some(expected.clone()))),
      Self::MissingScope { .. } => Some(GhAdvice::Refresh),
      Self::NotChecked
      | Self::SignedIn { .. }
      | Self::SignInUnknown
      | Self::ProgramMissing
      | Self::Unrecognized
      | Self::CouldNotRun => None,
    }
  }
}

/// A `gh` command the user runs in a terminal to change the sign-in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GhAdvice {
  /// Sign in on the host.
  Login,
  /// Make an account active on the host: the given one, or one the user picks.
  Switch(Option<GitHubLogin>),
  /// Add [`REQUIRED_SCOPE`] to the active account's sign-in.
  Refresh,
}

impl GhAdvice {
  /// The command line for `host`, e.g. `gh auth login --hostname github.com`.
  pub fn command(&self, host: &GitHubHost) -> String {
    match self {
      Self::Login => format!("gh auth login --hostname {host}"),
      Self::Switch(login) => {
        let login = login.as_ref().map_or(ANY_LOGIN, GitHubLogin::as_str);
        format!("gh auth switch --hostname {host} --user {login}")
      }
      Self::Refresh => format!("gh auth refresh -h {host} -s {REQUIRED_SCOPE}"),
    }
  }
}
