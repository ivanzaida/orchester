//! Why something asked of a server did not happen, and how the desktop says it. No variant holds a
//! token or a password: the client's errors never carry bodies or headers, and keychain errors never
//! carry values.

use orchester_api::{
  ErrorCode,
  client::{ClientError, ServerAddress},
};
use orchester_core::secret::VaultError;
use orchester_store::StoreError;
use thiserror::Error;

use crate::messages::{self, Message};

#[derive(Debug, Error)]
pub enum SharingError {
  #[error(transparent)]
  Client(#[from] ClientError),
  #[error("the application database failed: {0}")]
  Store(#[from] StoreError),
  #[error("the keychain failed: {0}")]
  Vault(#[from] VaultError),
  /// This computer has no token the server accepts: the person signs in again.
  #[error("not signed in to the server")]
  SignedOut,
  /// Work on a background thread ended before it finished; the cause is logged.
  #[error("the work was interrupted")]
  Interrupted,
}

impl From<Message> for SharingError {
  /// [`crate::background::run`] fails only when its work did not finish.
  fn from(_: Message) -> Self {
    Self::Interrupted
  }
}

impl SharingError {
  /// The server's error code, when it refused the request with one.
  pub fn code(&self) -> Option<ErrorCode> {
    match self {
      Self::Client(error) => error.code(),
      _ => None,
    }
  }

  /// What to tell the person, about the server at `address`.
  pub fn message(&self, address: &ServerAddress) -> Message {
    match self {
      Self::Client(error) => client_message(error, address),
      Self::Store(error) => messages::store_error(error),
      Self::Vault(error) => messages::vault_error(error),
      Self::SignedOut => Message::new("servers.errors.signedOut"),
      Self::Interrupted => messages::interrupted(),
    }
  }
}

/// What to tell the person about `error` from the server at `address`. Errors particular to a form
/// (a wrong password, a taken username) are said by the form itself; this is the general wording.
pub fn client_message(error: &ClientError, address: &ServerAddress) -> Message {
  let host = || address.host().to_owned();
  match error {
    ClientError::Address(_) => Message::new("servers.add.addressInvalid"),
    ClientError::PlainHttpRefused => Message::new("servers.errors.plainHttp").with("host", address.authority()),
    ClientError::Redirect { location } => Message::new("servers.check.redirect").with(
      "target",
      location.clone().unwrap_or_else(|| address.as_str().to_owned()),
    ),
    ClientError::TimedOut => Message::new("servers.check.timedOut").with("host", host()),
    ClientError::Unreachable => Message::new("servers.check.unreachable").with("host", host()),
    ClientError::UntrustedCertificate => Message::new(certificate_key()).with("host", host()),
    ClientError::NotOrchester { status } => Message::new("servers.check.notOrchester")
      .with("address", address.as_str().to_owned())
      .with("status", status.to_string()),
    ClientError::Api { code, .. } => api_message(*code),
    ClientError::Setup => Message::new("servers.errors.setup"),
  }
}

fn api_message(code: ErrorCode) -> Message {
  Message::new(match code {
    ErrorCode::Unauthorized => "servers.errors.signedOut",
    ErrorCode::InvalidCredentials => "servers.signIn.wrong",
    ErrorCode::PasswordChangeRequired => "servers.errors.passwordChangeRequired",
    ErrorCode::Forbidden => "servers.errors.forbidden",
    ErrorCode::NotFound => "servers.errors.notFound",
    ErrorCode::RateLimited => "servers.errors.rateLimited",
    _ => "servers.errors.refused",
  })
}

/// The certificate error names the trust store of the operating system.
#[cfg(windows)]
const fn certificate_key() -> &'static str {
  "servers.check.certificate.windows"
}

#[cfg(not(windows))]
const fn certificate_key() -> &'static str {
  "servers.check.certificate.macos"
}
