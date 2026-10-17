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
  #[error("the database failed: {0}")]
  Store(#[from] StoreError),
  #[error("the keychain failed: {0}")]
  Vault(#[from] VaultError),
  /// This computer has no token the server accepts: the person signs in again.
  #[error("not signed in to the server")]
  SignedOut,
  /// Work on a background thread ended before it finished; the cause is logged.
  #[error("the work was interrupted")]
  Interrupted,
  /// The server of a shared project cannot be reached now: its list is read-only and nothing is
  /// sent later without the person.
  #[error("the server cannot be reached; nothing was sent")]
  Offline,
  /// The person is no longer a member of the shared project; nothing was sent.
  #[error("no longer a member of the shared project")]
  NotMember,
  /// The shared project's server is no longer in the list of servers.
  #[error("the shared project's server is not in the list")]
  ServerMissing,
  /// The server answered with a value that breaks a rule of the task it describes.
  #[error("the server's answer has an invalid {0}")]
  InvalidAnswer(&'static str),
  /// The task kept changing on the server while Orchester merged a change into it; nothing more
  /// was sent.
  #[error("the task kept changing on the server")]
  KeptChanging,
  /// The project is being shared now; its tasks take changes again once it is.
  #[error("the project is being shared")]
  ShareInProgress,
  /// API 3 team servers have no passwords: devices join with a code or a cloud identity. Nothing was sent.
  #[error("the server has no passwords")]
  NoPasswords,
  /// A server whose own certificate this computer pinned presents another one; nothing was sent to it.
  #[error("the server presents a different certificate")]
  CertificateChanged,
  /// A code needs two-factor authentication on the person's Orchester Cloud account, which is off; nothing was sent.
  #[error("the cloud account has two-factor authentication off")]
  TwoFactorOff,
  /// A code needs this computer to have signed in to Orchester Cloud with two-factor authentication; nothing was sent.
  #[error("this computer signed in to Orchester Cloud without two-factor authentication")]
  TwoFactorSession,
  /// The server did not prove it holds the key whose id it gives itself: no token was asked for and nothing was sent.
  #[error("the server did not prove it holds the key of the id it gives itself")]
  UnprovenServer,
  /// The server has no project at the address, or the person is not a member of it; it answers both alike.
  #[error("no project the person is a member of at this address")]
  NoProject,
  /// The checkout's tasks are no longer those the person reviewed before confirming the link: nothing was sent, and
  /// the link is reviewed again.
  #[error("the project's tasks changed since the link was reviewed")]
  ProjectChanged,
  /// The server reported an address for one of its projects that is on another server, `own`: Orchester neither keeps
  /// it, writes it into a repository nor opens it. A server reached by another address than the one it names itself
  /// by does this; the person connects to it by `own`.
  #[error("the server reported a project address on another server, {own}")]
  ForeignAddress { own: ServerAddress },
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
      Self::Offline => Message::new("sharing.errors.offline").with("server", address.host().to_owned()),
      Self::NotMember => Message::new("sharing.errors.notMember"),
      Self::ServerMissing => Message::new("sharing.errors.serverMissing"),
      Self::InvalidAnswer(_) => Message::new("sharing.errors.invalidAnswer"),
      Self::KeptChanging => Message::new("sharing.errors.keptChanging"),
      Self::ShareInProgress => Message::new("sharing.errors.shareInProgress"),
      Self::NoPasswords => Message::new("servers.errors.noPasswords"),
      Self::CertificateChanged => {
        Message::new("servers.errors.certificateChanged").with("host", address.host().to_owned())
      }
      Self::NoProject => Message::new("sharing.errors.noProject"),
      Self::ProjectChanged => Message::new("sharing.errors.projectChanged"),
      Self::ForeignAddress { own } => Message::new("sharing.errors.foreignAddress")
        .with("server", address.as_str().to_owned())
        .with("own", own.as_str().to_owned()),
      Self::TwoFactorOff => Message::new("sharing.errors.twoFactorOff"),
      Self::TwoFactorSession => Message::new("sharing.errors.twoFactorSession"),
      Self::UnprovenServer => Message::new("sharing.errors.unprovenServer"),
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
    ClientError::Cloud { .. } => Message::new("servers.errors.cloudRefused"),
    ClientError::Setup => Message::new("servers.errors.setup"),
  }
}

fn api_message(code: ErrorCode) -> Message {
  Message::new(match code {
    ErrorCode::Unauthorized => "servers.errors.signedOut",
    ErrorCode::InvalidCode => "servers.join.invalidCode",
    ErrorCode::InvalidIdentity => "servers.join.invalidIdentity",
    ErrorCode::NotInvited => "servers.join.notInvited",
    ErrorCode::CloudOff => "servers.join.cloudOff",
    ErrorCode::KeySetStale => "servers.join.keySetStale",
    ErrorCode::ApprovalRefused => "servers.join.approvalRefused",
    ErrorCode::Forbidden => "servers.errors.forbidden",
    ErrorCode::NotFound => "servers.errors.notFound",
    ErrorCode::RateLimited => "servers.errors.rateLimited",
    ErrorCode::NotMember => "sharing.errors.notMember",
    ErrorCode::NotEditable => "sharing.errors.notEditable",
    ErrorCode::NotQueueable => "runs.start.incomplete",
    ErrorCode::Organization => "sharing.errors.organization",
    ErrorCode::RevisionConflict => "tasks.conflict.changedMeanwhile",
    ErrorCode::NoSuchUser => "sharing.members.noSuchUser",
    ErrorCode::AlreadyMember => "sharing.members.alreadyMember",
    ErrorCode::LastOwner => "sharing.members.lastOwner",
    ErrorCode::TagExists => "tasks.tags.duplicate",
    ErrorCode::SameTag => "tasks.tags.sameTarget",
    ErrorCode::ClaimHeld => "claims.refused.heldUnknown",
    ErrorCode::TaskMerged => "claims.refused.merged",
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
