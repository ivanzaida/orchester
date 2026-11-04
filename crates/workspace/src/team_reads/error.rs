//! Why a team project's read or subscription failed, typed so callers tell a server that cannot be reached from a
//! membership that ended or a session that changed. No variant carries a request, a response body or a token.

use orchester_api::{
  client::{AuthorityCallError, ClientError, SubscriptionEnd},
  v2::AuthorityErrorCode,
};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TeamReadError {
  /// The server cannot be reached now, or answered `unavailable` or `rate_limited`; `retry_after` in seconds when it
  /// said. Cached reads stay as they were, stale.
  #[error("the team server is unavailable")]
  Unavailable { retry_after: Option<u64> },
  /// `401`: the server no longer accepts this computer's token. Ends the project's reads.
  #[error("the team server signed this computer out")]
  SignedOut,
  /// The person is no longer a member of the workspace: `membership_revoked`, a `403` subscription or `revoked`.
  /// Ends the project's reads.
  #[error("the person is no longer a member of the workspace")]
  MembershipRevoked,
  /// The session the reads were made for ended: sign-out, another account or another server. Ends the project's
  /// reads; nothing read for the old session is returned.
  #[error("the session changed")]
  SessionChanged,
  /// `404`: the workspace, or the task, module or resource read, is not there for the caller.
  #[error("not found on the team server")]
  NotFound,
  /// A page cursor whose snapshot the server no longer keeps: read the listing again from its first page.
  #[error("the listing's snapshot expired")]
  CursorExpired,
  /// Every attempt at the read was invalidated while it was in flight: the data changed faster than it was read.
  #[error("the data kept changing while it was read")]
  KeptChanging,
  /// The server refused the read with another code.
  #[error("the team server refused the read: {0:?}")]
  Refused(AuthorityErrorCode),
  /// What answered is not an Orchester server, redirected, or could not be trusted.
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
      AuthorityCallError::Client(error) if error.is_connection() => Self::Unavailable { retry_after: None },
      AuthorityCallError::Client(error) => Self::Server(error),
      AuthorityCallError::Refused { error, retry_after, .. } => match error.code() {
        AuthorityErrorCode::Unauthenticated => Self::SignedOut,
        AuthorityErrorCode::MembershipRevoked => Self::MembershipRevoked,
        AuthorityErrorCode::NotFound => Self::NotFound,
        AuthorityErrorCode::CursorExpired => Self::CursorExpired,
        AuthorityErrorCode::Unavailable | AuthorityErrorCode::RateLimited => Self::Unavailable { retry_after },
        code => Self::Refused(code),
      },
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
