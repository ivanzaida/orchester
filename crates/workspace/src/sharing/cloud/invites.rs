//! The account's invites and team servers through Orchester Cloud (`docs/architecture/server.md`, "Invites", "New
//! computers"; `docs/cloud-api.md`): the invites waiting for the person, accepting one, declining one, and joining a
//! server the account belongs to. Accepting is in [`super::accepting`].

use orchester_api::{
  ErrorCode,
  client::{ClientError, ServerAddress},
  cloud::{AccountServer, CloudErrorCode, CloudInvite, CloudServer},
};
use orchester_core::id::{InviteId, TeamServerId};

use super::{Cloud, CloudError};
use crate::sharing::{CloudIdentity, CloudJoined, CodeRequest, SharingError, Trust};

/// How joining a team server with the account ended.
#[derive(Debug)]
pub enum ServerJoin {
  Joined(CloudJoined),
  /// The server knows no account of this cloud identity and has no open invite for its email.
  NotInvited,
}

impl Cloud {
  /// The invites waiting for the account, soonest to expire first, as the cloud lists them, without those accepted
  /// here (N1 design 3). The cloud lists an invite no more once it issued a token that accepts it; one whose first ask
  /// failed before that is still listed there, but this computer keeps it as accepted, to try again or set aside.
  pub async fn invites(&self) -> Result<Vec<CloudInvite>, CloudError> {
    let listed = self
      .with_token(|client, token| async move { Ok(client.invites(&token).await?) })
      .await?;
    let kept = self.accepted_invites().await?;
    Ok(
      listed
        .into_iter()
        .filter(|invite| !kept.iter().any(|accepted| accepted.invite == invite.id))
        .collect(),
    )
  }

  /// Declines `invite`: the cloud keeps it as declined, and no token can accept it any more. The team server is not
  /// told; its copy expires or is revoked there.
  pub async fn decline(&self, invite: InviteId) -> Result<(), CloudError> {
    self
      .with_token(|client, token| async move { ended(client.decline_invite(&token, invite).await) })
      .await?;
    self.changed();
    Ok(())
  }

  /// The team servers the account owns or joined, each with whether it owns it, as the cloud lists them
  /// (`docs/architecture/server.md`, row 44).
  pub async fn team_servers(&self) -> Result<Vec<AccountServer>, CloudError> {
    self
      .with_token(|client, token| async move { Ok(client.servers(&token).await?) })
      .await
  }

  /// Joins the team server at `address` with the account (*Connect a server › With my Orchester account*). The server
  /// names its id at `GET /server` (`linked`); the cloud's registry then gives its registered address and certificate,
  /// which must be the address the person typed. A server that names no id is looked for among the account's servers.
  pub async fn join_server(
    &self,
    address: &ServerAddress,
    linked: Option<TeamServerId>,
  ) -> Result<ServerJoin, CloudError> {
    let (client, token, account) = self.signed_in().await?;
    let server = match linked {
      Some(id) => {
        self
          .with_token(|client, token| async move { no_server(client.server(&token, id).await) })
          .await?
          .server
      }
      None => self
        .team_servers()
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
      account,
      invite: None,
    };
    match self.servers().join_with_cloud(&server, identity, None).await {
      Ok(joined) => Ok(ServerJoin::Joined(joined)),
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::NotInvited) => Ok(ServerJoin::NotInvited),
      Err(error) => Err(error.into()),
    }
  }
}

impl Cloud {
  /// Joins the team server at `address`, trusted as `trust`, with a code (*Connect a server › With a code*). A code
  /// needs this computer's cloud identity with two-factor authentication, asked for before the code is sent
  /// ([`crate::sharing::Servers::code_proof`]); a server that cannot check one is sent the code alone, and takes only
  /// its setup code then. The server goes into the list of the account whose sign-in asked for the proof, whoever is
  /// signed in once the join finished.
  pub async fn join_with_code(
    &self,
    address: &ServerAddress,
    trust: Trust,
    request: CodeRequest,
  ) -> Result<orchester_core::id::ServerId, CloudError> {
    let (client, token, account) = self.signed_in().await?;
    let identity = CloudIdentity {
      cloud: &client,
      device: &token,
      account,
      invite: None,
    };
    let proof = self.servers().code_proof(address, trust, identity).await?;
    let request = CodeRequest { proof, ..request };
    Ok(
      self
        .servers()
        .join_with_code_for(address, trust, request, Some(account))
        .await?,
    )
  }
}

/// Whether the cloud has `server` at `address`.
fn at(server: &CloudServer, address: &ServerAddress) -> bool {
  ServerAddress::parse(&server.address).is_ok_and(|known| known == *address)
}

/// `answer` about one invite: its `404` means the invite ended.
pub(super) fn ended<T>(answer: Result<T, ClientError>) -> Result<T, CloudError> {
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
