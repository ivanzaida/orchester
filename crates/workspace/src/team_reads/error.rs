//! Why a team project's read or subscription failed, typed so callers tell a server that cannot be reached from a
//! membership that ended or a session that changed. No variant carries a request, a response body or a token. The
//! team server answers `401` and `404` in the `/api/v1` `{error, message}` form and what is new to the reads
//! (`422 invalid`, `410 cursor_expired`, `426 unsupported_contract` with the version it needs) as an `AuthorityError`;
//! both forms are read.

use orchester_api::{
  ErrorCode,
  client::{AuthorityCallError, ClientError, SubscriptionEnd},
  releases::ReleaseVersion,
  v2::{AuthorityErrorCode, ErrorDetails},
};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TeamReadError {
  /// The server cannot be reached now, or answered `unavailable` or `rate_limited`; `retry_after` in seconds when it
  /// said. Cached reads stay as they were, stale.
  #[error("the team server is unavailable")]
  Unavailable { retry_after: Option<u64> },
  /// `401 unauthorized`: the server no longer accepts this computer's token. Ends the project's reads.
  #[error("the team server signed this computer out")]
  SignedOut,
  /// The person is no longer a member of the workspace: the subscription's `revoked`, or `membership_revoked`. Ends
  /// the project's reads.
  #[error("the person is no longer a member of the workspace")]
  MembershipRevoked,
  /// The session the reads were made for ended: sign-out, another account or another server. Ends the project's
  /// reads; nothing read for the old session is returned.
  #[error("the session changed")]
  SessionChanged,
  /// `404 not_found`: the workspace, or the task, module or resource read, is not there for the caller. The team
  /// server answers a removed member this way too.
  #[error("not found on the team server")]
  NotFound,
  /// A page cursor whose snapshot the server no longer keeps: read the listing again from its first page.
  #[error("the listing's snapshot expired")]
  CursorExpired,
  /// Every attempt at the read was invalidated while it was in flight: the data changed faster than it was read.
  #[error("the data kept changing while it was read")]
  KeptChanging,
  /// `426 unsupported_contract`: the server accepts desktops of `required` and later only, so this one needs an
  /// update before it reads the workspace again.
  #[error("the team server needs Orchester {required}")]
  NeedsVersion { required: ReleaseVersion },
  /// The server refused the read with another code.
  #[error("the team server refused the read: {0:?}")]
  Refused(AuthorityErrorCode),
  /// What answered is not an Orchester server, redirected, could not be trusted, or refused the read with another
  /// `/api/v1` code (e.g. `400 bad_request`).
  #[error(transparent)]
  Server(ClientError),
  /// The subscription sent an event the contract does not read.
  #[error("the team server sent an unreadable event")]
  Unreadable,
}

impl TeamReadError {
  /// Whether this error ends the project's reads and subscription for good.
  pub const fn ends_reads(&self) -> bool {
    matches!(self, Self::SignedOut | Self::MembershipRevoked | Self::SessionChanged)
  }
}

impl From<AuthorityCallError> for TeamReadError {
  fn from(error: AuthorityCallError) -> Self {
    match error {
      AuthorityCallError::Client(error) => error.into(),
      AuthorityCallError::Refused { error, retry_after, .. } => match (error.code(), error.details()) {
        (_, ErrorDetails::UnsupportedContract { required_version }) => Self::NeedsVersion {
          required: required_version.clone(),
        },
        (AuthorityErrorCode::Unauthenticated, _) => Self::SignedOut,
        (AuthorityErrorCode::MembershipRevoked, _) => Self::MembershipRevoked,
        (AuthorityErrorCode::NotFound, _) => Self::NotFound,
        (AuthorityErrorCode::CursorExpired, _) => Self::CursorExpired,
        (AuthorityErrorCode::Unavailable | AuthorityErrorCode::RateLimited, _) => Self::Unavailable { retry_after },
        (code, _) => Self::Refused(code),
      },
    }
  }
}

/// A failure of an `/api/v1` call: a server that cannot be reached, or the failures common to every route as the team
/// server answers them in that form.
impl From<ClientError> for TeamReadError {
  fn from(error: ClientError) -> Self {
    if error.is_connection() {
      return Self::Unavailable { retry_after: None };
    }
    match error {
      ClientError::Api {
        code: ErrorCode::Unauthorized,
        ..
      } => Self::SignedOut,
      ClientError::Api {
        code: ErrorCode::NotFound,
        ..
      } => Self::NotFound,
      error => Self::Server(error),
    }
  }
}

impl From<SubscriptionEnd> for TeamReadError {
  fn from(end: SubscriptionEnd) -> Self {
    match end {
      SubscriptionEnd::Closed => Self::Unavailable { retry_after: None },
      SubscriptionEnd::Unauthorized => Self::SignedOut,
      SubscriptionEnd::Revoked => Self::MembershipRevoked,
      SubscriptionEnd::NotFound => Self::NotFound,
      SubscriptionEnd::Failed(error) => error.into(),
      SubscriptionEnd::Unreadable => Self::Unreadable,
    }
  }
}
