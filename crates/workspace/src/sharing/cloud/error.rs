//! Why something asked of Orchester Cloud did not happen. No variant holds a token, a password or an email: the
//! client's errors never carry bodies or headers, and keychain errors never carry values.

use orchester_api::{
  client::{AddressError, ClientError},
  cloud::CloudErrorCode,
};
use orchester_core::secret::VaultError;
use orchester_store::StoreError;
use thiserror::Error;

use crate::{messages::Message, sharing::SharingError};

#[derive(Debug, Error)]
pub enum CloudError {
  #[error(transparent)]
  Client(#[from] ClientError),
  #[error("the database failed: {0}")]
  Store(#[from] StoreError),
  #[error("the keychain failed: {0}")]
  Vault(#[from] VaultError),
  /// The cloud's address in Settings › Orchester › Advanced is not one.
  #[error("not a cloud address: {0}")]
  Address(#[from] AddressError),
  /// Work on a background thread ended before it finished; the cause is logged.
  #[error("the work was interrupted")]
  Interrupted,
  /// No cloud account is signed in from this computer, or the cloud no longer accepts its token.
  #[error("not signed in to Orchester Cloud")]
  SignedOut,
  /// The invite ended before it was answered: accepted elsewhere, expired or revoked.
  #[error("the invite has ended")]
  InviteEnded,
  /// The team server refused this computer's join: its approval was refused, or the account there is disabled.
  #[error("the team server refused this computer")]
  JoinRefused,
  /// This computer asked for the invite as often as it does ([`super::MAX_ASKS`]); each ask takes an identity token.
  #[error("asked for this invite as often as Orchester does")]
  TooManyAsks,
  /// The cloud lists no team server at that address for the account.
  #[error("the cloud lists no team server at that address for the account")]
  NoServer,
  /// Joining the team server the cloud named failed.
  #[error(transparent)]
  Join(#[from] SharingError),
}

impl From<Message> for CloudError {
  /// [`crate::background::run`] fails only when its work did not finish.
  fn from(_: Message) -> Self {
    Self::Interrupted
  }
}

impl CloudError {
  /// The cloud's refusal code, when it refused the request with one.
  pub fn cloud_code(&self) -> Option<CloudErrorCode> {
    match self {
      Self::Client(ClientError::Cloud { code, .. }) => Some(*code),
      _ => None,
    }
  }

  /// How many seconds the cloud asked to wait, with `rate_limited` and `busy`.
  pub fn retry_after(&self) -> Option<u64> {
    match self {
      Self::Client(ClientError::Cloud { retry_after, .. }) => *retry_after,
      _ => None,
    }
  }

  /// Whether the cloud could not be reached at all.
  pub fn unreachable(&self) -> bool {
    matches!(self, Self::Client(error) if error.is_connection())
  }

  /// Whether the request went out and Orchester Cloud did not answer it: no connection, no answer in time, or
  /// something else answering in its place (a redirect, a certificate the system does not trust, another server), as
  /// on a network that needs a sign-in page first.
  pub fn unanswered(&self) -> bool {
    matches!(
      self,
      Self::Client(
        ClientError::TimedOut
          | ClientError::Unreachable
          | ClientError::NotOrchester { .. }
          | ClientError::Redirect { .. }
          | ClientError::UntrustedCertificate
      )
    )
  }

  /// Whether the cloud took the password but asks for a two-factor code, which the app does not ask for yet.
  pub fn two_factor_required(&self) -> bool {
    matches!(self, Self::Client(ClientError::TwoFactorRequired))
  }

  /// Why the cloud ended this computer's token, and when (milliseconds), when it refused it saying so.
  pub fn ended(&self) -> Option<(orchester_api::cloud::SessionEnd, Option<i64>)> {
    match self {
      Self::Client(ClientError::Cloud { ended, .. }) => *ended,
      _ => None,
    }
  }
}
