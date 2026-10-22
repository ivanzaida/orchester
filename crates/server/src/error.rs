use axum::{
  Json,
  http::StatusCode,
  response::{IntoResponse, Response},
};
use orchester_api::{ApiError, ErrorCode};
use thiserror::Error;

#[derive(Error)]
pub enum ServerError {
  #[error("cannot access server data")]
  Database(#[from] rusqlite::Error),
  #[error("cannot access server files")]
  Io(#[from] std::io::Error),
  #[error("invalid server configuration: {0}")]
  Configuration(&'static str),
  #[error("cannot generate credentials")]
  Random,
  #[error("server worker failed")]
  Worker,
  #[error("invalid stored server data")]
  StoredData,
  #[error("online backup did not complete; retry when the database is available")]
  BackupIncomplete,
  #[error("cannot load TLS certificate or key")]
  Tls,
  #[error("{0:?}")]
  Api(ErrorCode),
  #[error("shared project operation refused")]
  Sharing(Box<orchester_api::SharingError>),
  #[error("joining is rate limited")]
  RateLimited(u64),
}

impl From<ErrorCode> for ServerError {
  fn from(code: ErrorCode) -> Self {
    Self::Api(code)
  }
}

impl std::fmt::Debug for ServerError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // Sources can hold SQL values or filenames, so Debug follows the same redaction as operator logs.
    f.write_str(self.kind())
  }
}

impl ServerError {
  fn kind(&self) -> &'static str {
    match self {
      Self::Database(_) => "Database",
      Self::Io(_) => "Io",
      Self::Configuration(_) => "Configuration",
      Self::Random => "Random",
      Self::Worker => "Worker",
      Self::StoredData => "StoredData",
      Self::BackupIncomplete => "BackupIncomplete",
      Self::Tls => "Tls",
      Self::Api(_) => "Api",
      Self::Sharing(_) => "Sharing",
      Self::RateLimited(_) => "RateLimited",
    }
  }

  /// Logs structural diagnostics only; source messages may contain request data or secrets.
  pub fn log(&self) {
    match self {
      Self::Database(error) => {
        let sqlite = match error {
          rusqlite::Error::SqliteFailure(error, _) | rusqlite::Error::SqlInputError { error, .. } => Some(error),
          _ => None,
        };
        let message = if sqlite.is_some_and(|error| {
          matches!(
            error.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
          )
        }) {
          "server database is busy or locked; retry after the writer releases the lock"
        } else {
          "server operation failed"
        };
        tracing::error!(error_kind = self.kind(), database_variant = database_variant(error),
          sqlite_code = ?sqlite.map(|error| error.code), sqlite_extended_code = sqlite.map(|error| error.extended_code),
          "{message}");
      }
      Self::Io(error) => tracing::error!(error_kind = self.kind(), io_kind = ?error.kind(),
        os_error = error.raw_os_error(), "server operation failed"),
      Self::Configuration(reason) => tracing::error!(error_kind = self.kind(), reason, "server operation failed"),
      Self::Api(code) => tracing::error!(error_kind = self.kind(), code = ?code, "server operation failed"),
      _ => tracing::error!(error_kind = self.kind(), "server operation failed"),
    }
  }
}

fn database_variant(error: &rusqlite::Error) -> &'static str {
  use rusqlite::Error::*;
  match error {
    SqliteFailure(..) => "SqliteFailure",
    SqlInputError { .. } => "SqlInputError",
    SqliteSingleThreadedMode => "SqliteSingleThreadedMode",
    FromSqlConversionFailure(..) => "FromSqlConversionFailure",
    IntegralValueOutOfRange(..) => "IntegralValueOutOfRange",
    Utf8Error(..) => "Utf8Error",
    NulError(..) => "NulError",
    InvalidParameterName(..) => "InvalidParameterName",
    InvalidPath(..) => "InvalidPath",
    ExecuteReturnedResults => "ExecuteReturnedResults",
    QueryReturnedNoRows => "QueryReturnedNoRows",
    QueryReturnedMoreThanOneRow => "QueryReturnedMoreThanOneRow",
    InvalidColumnIndex(..) => "InvalidColumnIndex",
    InvalidColumnName(..) => "InvalidColumnName",
    InvalidColumnType(..) => "InvalidColumnType",
    StatementChangedRows(..) => "StatementChangedRows",
    ToSqlConversionFailure(..) => "ToSqlConversionFailure",
    InvalidQuery => "InvalidQuery",
    UnwindingPanic => "UnwindingPanic",
    MultipleStatement => "MultipleStatement",
    InvalidParameterCount(..) => "InvalidParameterCount",
    _ => "Other",
  }
}

impl IntoResponse for ServerError {
  fn into_response(self) -> Response {
    if let Self::Sharing(error) = self {
      return (status_message(error.error.error).0, Json(error)).into_response();
    }
    let (code, retry) = match self {
      Self::Api(code) => (code, None),
      Self::RateLimited(seconds) => (ErrorCode::RateLimited, Some(seconds)),
      error => {
        error.log();
        (ErrorCode::Internal, None)
      }
    };
    let (status, message) = status_message(code);
    let mut response = (
      status,
      Json(ApiError {
        error: code,
        message: message.into(),
      }),
    )
      .into_response();
    if let Some(seconds) = retry {
      response
        .headers_mut()
        .insert("Retry-After", seconds.to_string().parse().expect("integer header"));
    }
    response
  }
}

pub(crate) fn status_message(code: ErrorCode) -> (StatusCode, &'static str) {
  match code {
    ErrorCode::BadRequest => (StatusCode::BAD_REQUEST, "Malformed request"),
    ErrorCode::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large"),
    ErrorCode::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed"),
    ErrorCode::RequestTimeout => (StatusCode::REQUEST_TIMEOUT, "Request body timed out"),
    ErrorCode::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
    ErrorCode::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
    ErrorCode::PasswordChangeRequired => (StatusCode::FORBIDDEN, "Password change required"),
    ErrorCode::Forbidden => (StatusCode::FORBIDDEN, "Forbidden"),
    ErrorCode::NotFound => (StatusCode::NOT_FOUND, "Not found"),
    ErrorCode::Invalid => (StatusCode::UNPROCESSABLE_ENTITY, "Invalid input"),
    ErrorCode::UsernameTaken => (StatusCode::CONFLICT, "Username already exists"),
    ErrorCode::LastAdmin => (StatusCode::CONFLICT, "The last active admin must remain"),
    ErrorCode::NotMember => (StatusCode::FORBIDDEN, "Project membership required"),
    ErrorCode::NoSuchUser => (StatusCode::NOT_FOUND, "User not found"),
    ErrorCode::Conflict
    | ErrorCode::RemoteTaken
    | ErrorCode::AlreadyMember
    | ErrorCode::LastOwner
    | ErrorCode::IdTaken
    | ErrorCode::RevisionConflict
    | ErrorCode::ClaimHeld
    | ErrorCode::TaskMerged
    | ErrorCode::NotHolder
    | ErrorCode::ClaimNotStale
    | ErrorCode::ClaimChanged
    | ErrorCode::TagExists
    | ErrorCode::SameTag
    | ErrorCode::ModuleExists => (StatusCode::CONFLICT, "Conflicting project state"),
    ErrorCode::NotEditable | ErrorCode::NotQueueable | ErrorCode::Organization => {
      (StatusCode::UNPROCESSABLE_ENTITY, "Task change refused")
    }
    ErrorCode::Module => (StatusCode::UNPROCESSABLE_ENTITY, "Module change refused"),
    ErrorCode::InvalidIdentity => (StatusCode::UNAUTHORIZED, "Invalid identity token"),
    ErrorCode::InvalidCode => (StatusCode::UNAUTHORIZED, "Invalid code"),
    ErrorCode::NotInvited => (StatusCode::FORBIDDEN, "No account or invite for this identity"),
    ErrorCode::CloudOff => (
      StatusCode::FORBIDDEN,
      "This server does not accept Orchester Cloud identities",
    ),
    ErrorCode::ApprovalRefused => (StatusCode::FORBIDDEN, "The join was not approved"),
    ErrorCode::ApprovalRequired => (
      StatusCode::FORBIDDEN,
      "Join approval is on: sign in to the admin with a code from one of your computers",
    ),
    ErrorCode::MfaRequired => (
      StatusCode::FORBIDDEN,
      "Turn on two-factor authentication in Orchester Cloud and sign in with it to send invites",
    ),
    ErrorCode::MfaUnverifiable => (
      StatusCode::FORBIDDEN,
      "This server cannot check two-factor authentication: its admin pins Orchester Cloud's root key or links it",
    ),
    ErrorCode::AccountExists => (
      StatusCode::CONFLICT,
      "This Orchester Cloud identity has an account on this server already",
    ),
    ErrorCode::InviteOpen => (
      StatusCode::CONFLICT,
      "This address has an open invite already: revoke it to send a new one",
    ),
    ErrorCode::NothingToManage => (StatusCode::FORBIDDEN, "The account is not an admin and owns no project"),
    ErrorCode::KeySetStale => (
      StatusCode::CONFLICT,
      "The server's copy of Orchester Cloud's keys is out of date",
    ),
    ErrorCode::TooManyCodes => (StatusCode::CONFLICT, "Too many open codes"),
    ErrorCode::SlugTaken => (StatusCode::CONFLICT, "The slug is taken"),
    ErrorCode::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "Try again later"),
    ErrorCode::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Server operation failed"),
  }
}
