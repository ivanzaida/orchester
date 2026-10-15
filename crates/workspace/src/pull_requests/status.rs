//! What a sync of the pull requests came to, as the Pull requests page reports it under Refresh
//! (`docs/design/orchester-workspace.md`, "GitHub", rule 9; `Pull requests · sync states` in
//! `orchester2.pen`), and what the person can do about a failed one (`Pull requests · account states`).

use orchester_core::{
  connection::{GitHubLogin, GitHubState},
  pull_request::SyncFailure,
  repository::RepositoryError,
  time::Timestamp,
};

/// How the last sync went. It reports only what happened: `last` is when the list shown was read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncStatus {
  /// Nothing was read yet and nothing failed: the first sync has not answered.
  NotSynced,
  Synced(Timestamp),
  /// GitHub was not reached.
  Unreachable {
    last: Option<Timestamp>,
  },
  /// GitHub was reached but failed to answer (a server error).
  Troubled {
    last: Option<Timestamp>,
  },
  /// GitHub was reached but gave no list.
  Failed {
    last: Option<Timestamp>,
  },
  /// GitHub refuses requests until `resets_at`, when `gh` could tell.
  RateLimited {
    resets_at: Option<Timestamp>,
  },
  /// `gh` acts as `active`, another account than the project expects: sync is paused.
  Paused {
    active: GitHubLogin,
  },
  /// `gh` cannot act as the expected account for another reason.
  Account(GitHubState),
}

impl SyncStatus {
  /// After a sync that failed with `failure`, if it did; `last` is when what is shown was read.
  pub fn after(last: Option<Timestamp>, failure: Option<&SyncFailure>) -> Self {
    match failure {
      None => last.map_or(Self::NotSynced, Self::Synced),
      Some(SyncFailure::RateLimited { resets_at }) => Self::RateLimited { resets_at: *resets_at },
      Some(SyncFailure::Account(GitHubState::WrongAccount { active, .. })) => Self::Paused { active: active.clone() },
      Some(SyncFailure::Account(state)) => Self::Account(state.clone()),
      Some(SyncFailure::Repository(RepositoryError::Network { .. })) => Self::Unreachable { last },
      Some(SyncFailure::Repository(RepositoryError::ServerError { .. })) => Self::Troubled { last },
      Some(SyncFailure::Repository(_)) => Self::Failed { last },
    }
  }
}

/// What the notice of a failed sync offers besides its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Remedy {
  /// Nothing: Refresh, or the sync every minute, tries again.
  None,
  /// Check again, after a command run in a terminal.
  CheckAgain,
  /// Check again, or make the project expect `active`, the account `gh` acts as (*Use this account*).
  CheckAgainOrUseAccount { active: GitHubLogin },
  /// Check again, or fix the GitHub connection in Connections.
  CheckAgainOrConnections,
}

impl Remedy {
  pub fn of(failure: &SyncFailure) -> Self {
    match failure {
      SyncFailure::RateLimited { .. } => Self::None,
      SyncFailure::Account(GitHubState::WrongAccount { active, .. }) => {
        Self::CheckAgainOrUseAccount { active: active.clone() }
      }
      SyncFailure::Account(GitHubState::MissingScope { .. } | GitHubState::NotSignedIn) => Self::CheckAgain,
      SyncFailure::Account(_) => Self::CheckAgainOrConnections,
      SyncFailure::Repository(RepositoryError::NotSignedIn(_)) => Self::CheckAgain,
      SyncFailure::Repository(
        RepositoryError::NotFound(_) | RepositoryError::NoAccess(_) | RepositoryError::Run { .. },
      ) => Self::CheckAgainOrConnections,
      SyncFailure::Repository(_) => Self::None,
    }
  }
}
