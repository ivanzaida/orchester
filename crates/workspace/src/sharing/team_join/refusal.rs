//! Why joining a team did not happen: the refusals the start page and Settings say in the workspace's own words
//! ([`JoinRefusal::message`]), and every other failure as the cloud or the team server gave it. No variant holds a
//! code, a token or an email.

use orchester_api::{
  ErrorCode,
  client::{ClientError, ServerAddress},
};
use thiserror::Error;

use crate::{
  messages::Message,
  sharing::{SharingError, cloud::CloudError},
};

/// A join the team or the cloud refused for a reason the person can act on.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum JoinRefusal {
  /// The team server answers alike for an unknown, a used and an expired code (`invalid_code`), so as not to say
  /// which codes exist: one refusal for all three.
  #[error("the code is wrong, used or expired")]
  WrongCode,
  /// The Cloud invite expired, was revoked or was answered elsewhere.
  #[error("the invite has ended")]
  InviteEnded,
  /// Nothing answered at the team server's address.
  #[error("nothing answered at {address}")]
  Unreachable { address: ServerAddress },
  /// The team server has no shared project the person is a member of.
  #[error("{server} has no shared project for the person")]
  NoProjects { server: String },
}

impl JoinRefusal {
  /// What to tell the person.
  pub fn message(&self) -> Message {
    match self {
      Self::WrongCode => Message::new("teamJoin.refused.wrongCode"),
      Self::InviteEnded => Message::new("teamJoin.refused.inviteEnded"),
      Self::Unreachable { address } => {
        Message::new("teamJoin.refused.unreachable").with("address", address.as_str().to_owned())
      }
      Self::NoProjects { server } => Message::new("teamJoin.refused.noProjects").with("server", server.clone()),
    }
  }
}

/// Why a join, or the list of a team's projects, did not happen.
#[derive(Debug, Error)]
pub enum JoinError {
  #[error(transparent)]
  Refused(JoinRefusal),
  /// Orchester Cloud failed or refused, e.g. signed out ([`CloudError::SignedOut`]).
  #[error(transparent)]
  Cloud(CloudError),
  /// The team server, the database or the keychain failed otherwise.
  #[error(transparent)]
  Server(SharingError),
}

impl From<CloudError> for JoinError {
  /// A cloud failure before any team server was asked.
  fn from(error: CloudError) -> Self {
    Self::Cloud(error)
  }
}

impl From<SharingError> for JoinError {
  /// A failure before any team server was asked, e.g. of the database.
  fn from(error: SharingError) -> Self {
    Self::Server(error)
  }
}

/// `error` of the team server at `address`, as a refusal where it is one.
pub(super) fn from_sharing(error: SharingError, address: &ServerAddress) -> JoinError {
  match error {
    SharingError::Client(error) => from_client(error, address),
    error => JoinError::Server(error),
  }
}

/// `error` of a request to the team server at `address`.
pub(super) fn from_client(error: ClientError, address: &ServerAddress) -> JoinError {
  if error.code() == Some(ErrorCode::InvalidCode) {
    return JoinError::Refused(JoinRefusal::WrongCode);
  }
  if error.is_connection() {
    return JoinError::Refused(JoinRefusal::Unreachable {
      address: address.clone(),
    });
  }
  JoinError::Server(SharingError::Client(error))
}

/// `error` of a join through Orchester Cloud to the team server at `address`, `None` where the cloud named it at no
/// address there is. The cloud itself not answering stays the cloud's: it is not the team server.
pub(super) fn from_cloud(error: CloudError, address: Option<&ServerAddress>) -> JoinError {
  match (error, address) {
    (CloudError::InviteEnded, _) => JoinError::Refused(JoinRefusal::InviteEnded),
    (CloudError::Join(error), Some(address)) => from_sharing(error, address),
    (CloudError::Join(error), None) => JoinError::Server(error),
    (error, _) => JoinError::Cloud(error),
  }
}
