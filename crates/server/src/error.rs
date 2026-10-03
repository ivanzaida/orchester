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
  #[error("cannot hash or verify passwords")]
  PasswordHash,
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
  #[error("sign-in is rate limited")]
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
      Self::PasswordHash => "PasswordHash",
      Self::Worker => "Worker",
      Self::StoredData => "StoredData",
      Self::BackupIncomplete => "BackupIncomplete",
      Self::Tls => "Tls",
      Self::Api(_) => "Api",
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
    let (code, retry) = match self {
      Self::Api(code) => (code, None),
      Self::RateLimited(seconds) => (ErrorCode::RateLimited, Some(seconds)),
      error => {
        error.log();
        (ErrorCode::Internal, None)
      }
    };
    let (status, message) = match code {
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
      ErrorCode::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "Try again later"),
      ErrorCode::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Server operation failed"),
    };
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
