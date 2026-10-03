use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
  BadRequest,
  PayloadTooLarge,
  MethodNotAllowed,
  RequestTimeout,
  InvalidCredentials,
  Unauthorized,
  PasswordChangeRequired,
  Forbidden,
  NotFound,
  Invalid,
  UsernameTaken,
  LastAdmin,
  RateLimited,
  Internal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
  pub error: ErrorCode,
  /// English diagnostics; desktop text is translated by error code.
  pub message: String,
}
