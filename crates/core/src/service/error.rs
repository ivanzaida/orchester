use thiserror::Error;

use super::{
  name::{ServiceName, StackName},
  readiness::{LogPattern, ReadyTimeout},
};
use crate::resource::FieldName;

/// Why a part of a service or stack definition is not valid. Values that may hold a secret by mistake (a plain
/// variable value, a request body) are never repeated here, only their names.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ServiceError {
  #[error(
    "`{0}` is not a service or stack name: 1–{max} lowercase letters, digits and single hyphens",
    max = ServiceName::MAX_CHARS
  )]
  Name(String),
  #[error("`{0}` is not a folder inside the worktree: a relative path without `..`, a drive or a leading separator")]
  OutsideWorktree(String),
  #[error("the run's data folder must be a folder below the worktree, not the worktree itself")]
  RunFolderIsRoot,
  #[error("`{0}` is not a path pattern inside the repository")]
  OwnedPath(String),
  #[error("the owned path `{0}` is listed twice")]
  DuplicateOwnedPath(String),
  /// Windows does not tell names apart by case, so neither does a service.
  #[error("the variable `{0}` is set twice")]
  DuplicateVariable(String),
  #[error("the value of `{0}` contains a NUL character")]
  VariableValue(String),
  #[error("the service `{0}` needs itself")]
  NeedsItself(ServiceName),
  #[error("the service `{0}` is needed twice")]
  DuplicateNeed(ServiceName),
  #[error("`{0}` is not an HTTP path: it starts with `/` and has no spaces or control characters")]
  HttpPath(String),
  #[error("the log pattern is longer than {max} characters", max = LogPattern::MAX_CHARS)]
  LogPatternTooLong,
  #[error("`{pattern}` is not a regular expression: {reason}")]
  LogPattern { pattern: String, reason: String },
  #[error("the readiness timeout must be between 1 and {max} seconds", max = ReadyTimeout::MAX_SECS)]
  ReadyTimeout,
  #[error("`{0}` is not a request URL: an http(s) URL or a placeholder, without spaces or control characters")]
  SeedUrl(String),
  #[error("`{0}` is not an HTTP header name")]
  HeaderName(String),
  #[error("the header `{0}` is set twice")]
  DuplicateHeader(String),
  #[error("the value of the header `{0}` contains a control character such as a line break")]
  HeaderValue(String),
  #[error("`{0}` is not a JSON path such as `$.id` or `$.items[0].id`")]
  CapturePath(String),
  #[error("the seed captures `{0}` twice")]
  DuplicateCapture(FieldName),
  #[error("a seed of HTTP requests needs at least one request")]
  NoSeedRequests,
  #[error("the stack `{0}` has no services")]
  EmptyStack(StackName),
  #[error("the stack `{stack}` lists `{service}` twice")]
  DuplicateStackService { stack: StackName, service: ServiceName },
}
