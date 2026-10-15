//! `pull_request_attempts`: how the latest sync of a repository went. A failure is kept as what the Pull
//! requests page and the domain tools say about it (the kind, the accounts, a rate limit's reset, an HTTP
//! status); the text of an error and a program's output are the log's. A program that could not run is kept
//! as `gh` that could not run.

use orchester_core::{
  connection::{GitHubLogin, GitHubState},
  pull_request::{SyncAttempt, SyncFailure},
  repository::{GitHubRepo, RepositoryError},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{StoreError, rows::corrupt};

/// A failure as its row keeps it.
#[derive(Default)]
struct Row {
  failure: Option<&'static str>,
  account_state: Option<&'static str>,
  login: Option<String>,
  expected_login: Option<String>,
  resets_at: Option<i64>,
  http_status: Option<u16>,
}

pub(super) fn record(connection: &Connection, repo: &GitHubRepo, attempt: &SyncAttempt) -> Result<(), StoreError> {
  let row = attempt.failure.as_ref().map_or_else(Row::default, encode);
  connection.execute(
    "INSERT INTO pull_request_attempts \
       (host, owner, name, attempted_at, failure, account_state, login, expected_login, resets_at, http_status) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
     ON CONFLICT (host, owner, name) DO UPDATE SET attempted_at = excluded.attempted_at, \
       failure = excluded.failure, account_state = excluded.account_state, login = excluded.login, \
       expected_login = excluded.expected_login, resets_at = excluded.resets_at, \
       http_status = excluded.http_status",
    params![
      repo.host().as_str(),
      repo.owner(),
      repo.name(),
      attempt.at.unix_millis(),
      row.failure,
      row.account_state,
      row.login,
      row.expected_login,
      row.resets_at,
      row.http_status,
    ],
  )?;
  Ok(())
}

pub(super) fn latest(connection: &Connection, repo: &GitHubRepo) -> Result<Option<SyncAttempt>, StoreError> {
  type Stored = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<u16>,
  );
  let stored: Option<Stored> = connection
    .query_row(
      "SELECT attempted_at, failure, account_state, login, expected_login, resets_at, http_status \
       FROM pull_request_attempts WHERE host = ?1 AND owner = ?2 AND name = ?3",
      params![repo.host().as_str(), repo.owner(), repo.name()],
      |row| {
        Ok((
          row.get(0)?,
          row.get(1)?,
          row.get(2)?,
          row.get(3)?,
          row.get(4)?,
          row.get(5)?,
          row.get(6)?,
        ))
      },
    )
    .optional()?;
  let Some((at, failure, account_state, login, expected_login, resets_at, http_status)) = stored else {
    return Ok(None);
  };
  let failure = match failure.as_deref() {
    None => None,
    Some(kind) => Some(decode(
      repo,
      kind,
      (account_state.as_deref(), login, expected_login),
      (resets_at, http_status),
    )?),
  };
  Ok(Some(SyncAttempt {
    at: Timestamp::from_unix_millis(at),
    failure,
  }))
}

fn encode(failure: &SyncFailure) -> Row {
  match failure {
    SyncFailure::RateLimited { resets_at } => Row {
      failure: Some("rate_limited"),
      resets_at: resets_at.map(Timestamp::unix_millis),
      ..Row::default()
    },
    SyncFailure::Account(state) => account(state),
    SyncFailure::Repository(error) => repository(error),
  }
}

fn account(state: &GitHubState) -> Row {
  let (kind, login, expected) = match state {
    GitHubState::NotChecked => ("not_checked", None, None),
    GitHubState::SignedIn { account } => ("signed_in", Some(account), None),
    GitHubState::NotSignedIn => ("not_signed_in", None, None),
    GitHubState::SignInUnknown => ("sign_in_unknown", None, None),
    GitHubState::WrongAccount { active, expected } => ("wrong_account", Some(active), Some(expected)),
    GitHubState::MissingScope { account } => ("missing_scope", Some(account), None),
    GitHubState::ProgramMissing => ("program_missing", None, None),
    GitHubState::Unrecognized => ("unrecognized", None, None),
    GitHubState::CouldNotRun => ("could_not_run", None, None),
  };
  Row {
    failure: Some("account"),
    account_state: Some(kind),
    login: login.map(|login| login.as_str().to_owned()),
    expected_login: expected.map(|login| login.as_str().to_owned()),
    ..Row::default()
  }
}

fn repository(error: &RepositoryError) -> Row {
  let kind = match error {
    RepositoryError::Network { .. } => "unreachable",
    RepositoryError::ServerError { status, .. } => {
      return Row {
        failure: Some("server_error"),
        http_status: *status,
        ..Row::default()
      };
    }
    RepositoryError::NotFound(_) => "not_found",
    RepositoryError::NoAccess(_) => "no_access",
    RepositoryError::NotSignedIn(_) => "not_signed_in",
    RepositoryError::Run { .. } => return account(&GitHubState::CouldNotRun),
    _ => "unrecognized",
  };
  Row {
    failure: Some(kind),
    ..Row::default()
  }
}

fn decode(
  repo: &GitHubRepo,
  kind: &str,
  (account_state, login, expected): (Option<&str>, Option<String>, Option<String>),
  (resets_at, http_status): (Option<i64>, Option<u16>),
) -> Result<SyncFailure, StoreError> {
  let host = || repo.host().clone();
  Ok(match kind {
    "rate_limited" => SyncFailure::RateLimited {
      resets_at: resets_at.map(Timestamp::from_unix_millis),
    },
    "account" => SyncFailure::Account(state(account_state, login, expected)?),
    "unreachable" => SyncFailure::Repository(RepositoryError::Network {
      host: host(),
      detail: String::new(),
    }),
    "server_error" => SyncFailure::Repository(RepositoryError::ServerError {
      host: host(),
      status: http_status,
      detail: String::new(),
    }),
    "not_found" => SyncFailure::Repository(RepositoryError::NotFound(repo.clone())),
    "no_access" => SyncFailure::Repository(RepositoryError::NoAccess(repo.clone())),
    "not_signed_in" => SyncFailure::Repository(RepositoryError::NotSignedIn(host())),
    "unrecognized" => SyncFailure::Repository(RepositoryError::Unrecognized {
      program: "gh",
      detail: String::new(),
    }),
    other => return Err(corrupt("pull request sync failure", other)),
  })
}

fn state(kind: Option<&str>, login: Option<String>, expected: Option<String>) -> Result<GitHubState, StoreError> {
  let login_of = |value: Option<String>| {
    value
      .as_deref()
      .map(GitHubLogin::new)
      .transpose()
      .map_err(|error| corrupt("pull request sync account", error))?
      .ok_or_else(|| corrupt("pull request sync account", "missing"))
  };
  Ok(match kind {
    Some("not_checked") => GitHubState::NotChecked,
    Some("signed_in") => GitHubState::SignedIn {
      account: login_of(login)?,
    },
    Some("not_signed_in") => GitHubState::NotSignedIn,
    Some("sign_in_unknown") => GitHubState::SignInUnknown,
    Some("wrong_account") => GitHubState::WrongAccount {
      active: login_of(login)?,
      expected: login_of(expected)?,
    },
    Some("missing_scope") => GitHubState::MissingScope {
      account: login_of(login)?,
    },
    Some("program_missing") => GitHubState::ProgramMissing,
    Some("unrecognized") => GitHubState::Unrecognized,
    Some("could_not_run") => GitHubState::CouldNotRun,
    other => return Err(corrupt("pull request sync account state", format!("{other:?}"))),
  })
}
