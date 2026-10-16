use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use orchester_api::{
  cloud::{CloudError as ApiError, CloudErrorCode},
  identity::IdentityError,
};
use thiserror::Error;

#[derive(Error)]
pub enum CloudError {
  #[error("cannot access the cloud's database")]
  Database(#[from] tokio_postgres::Error),
  #[error("cannot get a database connection")]
  Pool,
  #[error("the database is at schema version {0}, newer than this binary knows")]
  SchemaTooNew(i32),
  #[error("cannot access files")]
  Io(#[from] std::io::Error),
  #[error("invalid configuration: {0}")]
  Configuration(&'static str),
  #[error("cannot generate random data")]
  Random,
  #[error("cannot hash or verify passwords")]
  PasswordHash,
  #[error("cloud worker failed")]
  Worker,
  #[error("invalid stored data")]
  StoredData,
  #[error("cannot seal or open a root or signing key")]
  KeyEncryption,
  #[error("the key-encryption key does not open the root key in the database")]
  WrongKeyEncryptionKey,
  #[error("the database holds signing keys or key sets but no root key")]
  KeysWithoutRoot,
  #[error("identity token or key set refused: {0}")]
  Identity(#[from] IdentityError),
  #[error("cannot send email")]
  Mail,
  #[error("{0:?}")]
  Api(CloudErrorCode),
  #[error("rate limited")]
  RateLimited(u64),
}

impl From<CloudErrorCode> for CloudError {
  fn from(code: CloudErrorCode) -> Self {
    Self::Api(code)
  }
}

impl From<deadpool_postgres::PoolError> for CloudError {
  fn from(error: deadpool_postgres::PoolError) -> Self {
    match error {
      deadpool_postgres::PoolError::Backend(error) => Self::Database(error),
      _ => Self::Pool,
    }
  }
}

impl std::fmt::Debug for CloudError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // Sources can hold SQL values or connection details, so Debug shows the kind only, as the log does.
    f.write_str(self.kind())
  }
}

/// What the log says when the key-encryption key does not open the stored root: the cloud never makes another root,
/// so the operator restores the key this database started with (docs/cloud-hosting.md).
const WRONG_KEY_ENCRYPTION_KEY: &str =
  "ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY does not open the root key in the database; set the key it was made with";

/// What the log says when the root row is gone but keys it vouched for are not: the cloud does not make a root
/// beside them; emptying root_key, signing_keys and key_sets together starts over (docs/cloud-hosting.md).
const KEYS_WITHOUT_ROOT: &str =
  "the database holds signing keys or key sets but no root key: restore it, or empty signing_keys and key_sets too";

impl CloudError {
  fn kind(&self) -> &'static str {
    match self {
      Self::Database(_) => "Database",
      Self::Pool => "Pool",
      Self::SchemaTooNew(_) => "SchemaTooNew",
      Self::Io(_) => "Io",
      Self::Configuration(_) => "Configuration",
      Self::Random => "Random",
      Self::PasswordHash => "PasswordHash",
      Self::Worker => "Worker",
      Self::StoredData => "StoredData",
      Self::KeyEncryption => "KeyEncryption",
      Self::WrongKeyEncryptionKey => "WrongKeyEncryptionKey",
      Self::KeysWithoutRoot => "KeysWithoutRoot",
      Self::Identity(_) => "Identity",
      Self::Mail => "Mail",
      Self::Api(_) => "Api",
      Self::RateLimited(_) => "RateLimited",
    }
  }

  /// Logs structural diagnostics only: database messages and details can hold submitted values.
  pub fn log(&self) {
    let detail = match self {
      Self::Database(error) => error.code().map(|state| format!("sql_state={}", state.code())),
      Self::Io(error) => Some(format!("io_kind={:?}", error.kind())),
      Self::Configuration(reason) => Some((*reason).to_owned()),
      Self::WrongKeyEncryptionKey => Some(WRONG_KEY_ENCRYPTION_KEY.to_owned()),
      Self::KeysWithoutRoot => Some(KEYS_WITHOUT_ROOT.to_owned()),
      Self::SchemaTooNew(version) => Some(format!("schema_version={version}")),
      Self::Identity(error) => Some(error.to_string()),
      Self::Api(code) => Some(format!("code={code:?}")),
      _ => None,
    };
    tracing::error!(error_kind = self.kind(), detail, "cloud operation failed");
  }

  fn code(&self) -> CloudErrorCode {
    match self {
      Self::Api(code) => *code,
      Self::RateLimited(_) => CloudErrorCode::RateLimited,
      _ => CloudErrorCode::Internal,
    }
  }
}

impl ResponseError for CloudError {
  fn status_code(&self) -> StatusCode {
    status_message(self.code()).0
  }

  fn error_response(&self) -> HttpResponse {
    let code = self.code();
    if code == CloudErrorCode::Internal {
      self.log();
    }
    let (status, message) = status_message(code);
    let mut response = HttpResponse::build(status);
    if let Self::RateLimited(seconds) = self {
      response.insert_header(("Retry-After", seconds.to_string()));
    }
    if code == CloudErrorCode::Busy {
      response.insert_header(("Retry-After", "1"));
    }
    response.json(ApiError {
      error: code,
      message: message.into(),
    })
  }
}

fn status_message(code: CloudErrorCode) -> (StatusCode, &'static str) {
  match code {
    CloudErrorCode::BadRequest => (StatusCode::BAD_REQUEST, "Malformed request"),
    CloudErrorCode::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large"),
    CloudErrorCode::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed"),
    CloudErrorCode::RequestTimeout => (StatusCode::REQUEST_TIMEOUT, "Request body timed out"),
    CloudErrorCode::UnsupportedMediaType => (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Request body must be JSON"),
    CloudErrorCode::NotFound => (StatusCode::NOT_FOUND, "Not found"),
    CloudErrorCode::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
    CloudErrorCode::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
    CloudErrorCode::EmailUnconfirmed => (StatusCode::FORBIDDEN, "Email not confirmed"),
    CloudErrorCode::InvalidLink => (StatusCode::UNPROCESSABLE_ENTITY, "Invalid or expired link"),
    CloudErrorCode::Invalid => (StatusCode::UNPROCESSABLE_ENTITY, "Invalid input"),
    CloudErrorCode::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "Try again later"),
    CloudErrorCode::KeysUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "Signing keys unavailable"),
    CloudErrorCode::Busy => (StatusCode::SERVICE_UNAVAILABLE, "Busy, try again shortly"),
    CloudErrorCode::Conflict => (
      StatusCode::CONFLICT,
      "The server is linked by a newer statement, or this one was used",
    ),
    CloudErrorCode::TooManyServers => (StatusCode::CONFLICT, "The account owns as many servers as it may"),
    CloudErrorCode::MfaRequired => (StatusCode::FORBIDDEN, "Two-factor authentication required"),
    CloudErrorCode::SessionWithoutMfa => (StatusCode::FORBIDDEN, "This session signed in without two-factor"),
    CloudErrorCode::InvalidCode => (StatusCode::UNAUTHORIZED, "Invalid code"),
    CloudErrorCode::ChallengeExpired => (StatusCode::UNAUTHORIZED, "Sign in again"),
    CloudErrorCode::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Cloud operation failed"),
  }
}
