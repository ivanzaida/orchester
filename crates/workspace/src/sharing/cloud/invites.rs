//! The account's invites and team servers through Orchester Cloud (`docs/architecture/server.md`, "Invites", "New
//! computers"): the invites waiting for the person, accepting one, which connects this computer to the invite's server
//! trusting exactly the certificate the invite names (no check code to compare), declining one, and joining a server
//! the account already belongs to. A cloud that does not serve invites or servers yet says so
//! ([`CloudError::NotOffered`]).

use orchester_api::{
  ErrorCode,
  client::{ClientError, ServerAddress},
  cloud::{CloudErrorCode, CloudInvite, CloudServer, InvitedProject},
};
use orchester_core::id::InviteId;

use super::{Cloud, CloudError, error::offered};
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
  /// The invites waiting for the account, newest first.
  pub async fn invites(&self) -> Result<Vec<CloudInvite>, CloudError> {
    let mut invites = self
      .with_token(|client, token| async move { offered(client.invites(&token).await) })
      .await?;
    invites.sort_by_key(|invite| std::cmp::Reverse(invite.created_at));
    Ok(invites)
  }

  /// Accepts `invite`: the cloud names its server and gives a one-time identity token for it, and this computer joins
  /// the server at that address, trusting only the certificate the invite names.
  pub async fn accept(&self, invite: InviteId) -> Result<Accepted, CloudError> {
    let (accepted, key_set) = self
      .with_token(|client, token| async move {
        let accepted = ended(client.accept_invite(&token, invite).await)?;
        let key_set = client.key_set().await?.key_set;
        Ok((accepted, key_set))
      })
      .await?;
    let joined = self
      .servers()
      .join_with_invite_token(&accepted.server, accepted.identity_token, key_set)
      .await;
    self.changed();
    Ok(Accepted {
      joined: joined?,
      server: accepted.server,
      project: accepted.project,
    })
  }

  /// Declines `invite`: it ends, and whoever invited sees it was declined.
  pub async fn decline(&self, invite: InviteId) -> Result<(), CloudError> {
    self
      .with_token(|client, token| async move { ended(client.decline_invite(&token, invite).await) })
      .await?;
    self.changed();
    Ok(())
  }

  /// Joins the team server at `address` with the account (*Connect a server › With my Orchester account*): the cloud
  /// lists the servers the account owns or joined, and the join goes to that server as the cloud names it.
  pub async fn join_server(&self, address: &ServerAddress) -> Result<ServerJoin, CloudError> {
    let (client, token) = self.signed_in().await?;
    let servers = self
      .with_token(|client, token| async move { offered(client.servers(&token).await) })
      .await?;
    let server = servers
      .into_iter()
      .find(|server| ServerAddress::parse(&server.address).is_ok_and(|known| known == *address))
      .ok_or(CloudError::NoServer)?;
    let identity = CloudIdentity {
      cloud: &client,
      device: &token,
    };
    match self.servers().join_with_cloud(&server, identity, None).await {
      Ok(joined) => Ok(ServerJoin::Joined(joined)),
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::NotInvited) => Ok(ServerJoin::NotInvited),
      Err(error) => Err(error.into()),
    }
  }
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
