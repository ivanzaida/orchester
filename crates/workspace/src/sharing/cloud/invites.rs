//! The account's invites and team servers through Orchester Cloud (`docs/architecture/server.md`, "Invites", "New
//! computers"; `docs/cloud-api.md`): the invites waiting for the person, accepting one, declining one, and joining a
//! server the account belongs to. Accepting reads the invite's server from the cloud's registry, which holds the
//! certificate fingerprint the server linked with, and joins it with an identity token that names the invite: this
//! computer trusts only that certificate there, so there is no check code to compare.

use orchester_api::{
  ErrorCode,
  client::{ClientError, ServerAddress},
  cloud::{CloudErrorCode, CloudInvite, CloudServer},
  statements::InvitedProject,
};
use orchester_core::id::{InviteId, TeamServerId};

use super::{Cloud, CloudError};
use crate::sharing::{CloudIdentity, CloudJoined, SharingError};

/// An invite accepted: the server joined (or waiting for approval) and the project it was to, if any.
#[derive(Debug)]
pub struct Accepted {
  pub server: CloudServer,
  pub project: Option<InvitedProject>,
  pub joined: CloudJoined,
}

/// How joining a team server with the account ended.
#[derive(Debug)]
pub enum ServerJoin {
  Joined(CloudJoined),
  /// The server knows no account of this cloud identity and has no open invite for its email.
  NotInvited,
}

impl Cloud {
  /// The invites waiting for the account, soonest to expire first, as the cloud lists them.
  pub async fn invites(&self) -> Result<Vec<CloudInvite>, CloudError> {
    self
      .with_token(|client, token| async move { Ok(client.invites(&token).await?) })
      .await
  }

  /// Accepts `invite`: this computer joins the invite's server at the address and with the certificate the cloud's
  /// registry has for it, with an identity token that names the invite. The cloud's copy of the invite ends then; the
  /// server's when the token joins.
  pub async fn accept(&self, invite: InviteId) -> Result<Accepted, CloudError> {
    let (client, token) = self.signed_in().await?;
    let listed = self
      .invites()
      .await?
      .into_iter()
      .find(|listed| listed.id == invite)
      .ok_or(CloudError::InviteEnded)?;
    let server = self
      .with_token(|client, token| async move { ended(client.server(&token, listed.server.id).await) })
      .await?
      .server;
    let identity = CloudIdentity {
      cloud: &client,
      device: &token,
      invite: Some(invite),
    };
    let joined = self.servers().join_with_cloud(&server, identity, None).await;
    self.changed();
    let joined = joined.map_err(|error| match error {
      // The cloud no longer has the invite open: declined elsewhere, revoked or expired.
      SharingError::Client(ClientError::Cloud {
        code: CloudErrorCode::NotFound,
        ..
      }) => CloudError::InviteEnded,
      error => error.into(),
    })?;
    Ok(Accepted {
      joined,
      server,
      project: listed.project,
    })
  }

  /// Declines `invite`: the cloud's copy ends and no token can accept it any more. The team server is not told; its
  /// copy expires or is revoked there.
  pub async fn decline(&self, invite: InviteId) -> Result<(), CloudError> {
    self
      .with_token(|client, token| async move { ended(client.decline_invite(&token, invite).await) })
      .await?;
    self.changed();
    Ok(())
  }

  /// Joins the team server at `address` with the account (*Connect a server › With my Orchester account*). The server
  /// names its id at `GET /server` (`linked`); the cloud's registry then gives its registered address and certificate,
  /// which must be the address the person typed. A server that names no id is looked for among the account's servers.
  pub async fn join_server(
    &self,
    address: &ServerAddress,
    linked: Option<TeamServerId>,
  ) -> Result<ServerJoin, CloudError> {
    let (client, token) = self.signed_in().await?;
    let server = match linked {
      Some(id) => {
        self
          .with_token(|client, token| async move { no_server(client.server(&token, id).await) })
          .await?
          .server
      }
      None => self
        .with_token(|client, token| async move { Ok(client.servers(&token).await?) })
        .await?
        .into_iter()
        .map(|listed| listed.server)
        .find(|server| at(server, address))
        .ok_or(CloudError::NoServer)?,
    };
    if !at(&server, address) {
      return Err(CloudError::NoServer);
    }
    let identity = CloudIdentity {
      cloud: &client,
      device: &token,
      invite: None,
    };
    match self.servers().join_with_cloud(&server, identity, None).await {
      Ok(joined) => Ok(ServerJoin::Joined(joined)),
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::NotInvited) => Ok(ServerJoin::NotInvited),
      Err(error) => Err(error.into()),
    }
  }
}

/// Whether the cloud has `server` at `address`.
fn at(server: &CloudServer, address: &ServerAddress) -> bool {
  ServerAddress::parse(&server.address).is_ok_and(|known| known == *address)
}

/// `answer` about one invite: its `404` means the invite ended.
fn ended<T>(answer: Result<T, ClientError>) -> Result<T, CloudError> {
  answer.map_err(|error| match error {
    ClientError::Cloud {
      code: CloudErrorCode::NotFound,
      ..
    } => CloudError::InviteEnded,
    error => error.into(),
  })
}

/// `answer` about a server in the registry: its `404` means the cloud has no such linked server.
fn no_server<T>(answer: Result<T, ClientError>) -> Result<T, CloudError> {
  answer.map_err(|error| match error {
    ClientError::Cloud {
      code: CloudErrorCode::NotFound,
      ..
    } => CloudError::NoServer,
    error => error.into(),
  })
}
