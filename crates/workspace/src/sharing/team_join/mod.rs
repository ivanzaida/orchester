//! Joining a team with no project open (`docs/design/startup.md`, decisions 4–5; `docs/architecture/server.md`, rows
//! 43–45 and 50), for the start page and Settings › Orchester alike: the Orchester Cloud account's team servers and
//! invites, joining one of them, which gives this computer its own device sign-in there, joining by a server's address
//! and a code with or without Cloud, and then the team's shared projects the person is invited to. Everything here
//! uses only the application database and the keychain; the joins themselves are [`Servers`]' and [`Cloud`]'s.

mod refusal;

use orchester_api::{
  Approval,
  client::{ClientError, PlainHttp, ServerAddress},
  cloud::{CloudInvite, CloudServer},
};
use orchester_core::id::{ServerId, TeamServerId};

pub use self::refusal::{JoinError, JoinRefusal};
use super::{
  CloudJoined, CodeRequest, Reached, ServerEntry, ServerProject, Servers, SharingError, Trust,
  cloud::{Cloud, CloudError, ServerJoin},
};
use crate::background;

/// The session's team joins. Clones share its servers and cloud account.
#[derive(Clone)]
pub struct TeamJoin {
  servers: Servers,
  cloud: Cloud,
}

/// What the Orchester Cloud account can join: its team servers and the invites waiting for it.
#[derive(Clone, Debug)]
pub struct Teams {
  pub servers: Vec<TeamServer>,
  pub invites: Vec<CloudInvite>,
}

/// A team server of the account, and this computer's entry of it once it joined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamServer {
  pub server: CloudServer,
  pub joined: Option<ServerId>,
}

/// The server joined, with the name and the username it has there now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinedServer {
  pub id: ServerId,
  pub name: String,
  pub username: String,
}

/// How a join ended.
#[derive(Clone, Debug)]
pub enum JoinOutcome {
  Joined(JoinedServer),
  /// The server holds the join until another computer of the person's, or an admin, approves it.
  Waiting(Approval),
  /// The server has no place for the account: no account of its cloud identity there and no invite for its email.
  NotInvited,
}

impl TeamJoin {
  pub fn new(servers: Servers, cloud: Cloud) -> Self {
    Self { servers, cloud }
  }

  /// The account's team servers, each with this computer's entry when it is signed in there, and its invites;
  /// [`CloudError::SignedOut`] without a Cloud sign-in.
  pub async fn teams(&self) -> Result<Teams, JoinError> {
    let listed = self.cloud.team_servers().await?;
    let invites = self.cloud.invites().await?;
    let entries = self.entries().await?;
    let servers = listed
      .into_iter()
      .map(|server| {
        // A server the cloud names at no address there is cannot be in the list: none is added without one.
        let address = ServerAddress::parse(&server.address).ok();
        let joined = entries
          .iter()
          .find(|entry| Some(&entry.address) == address.as_ref() && entry.in_use())
          .map(|entry| entry.stored.id);
        TeamServer { server, joined }
      })
      .collect();
    Ok(Teams { servers, invites })
  }

  /// Joins `server`, one of the account's team servers, with the account.
  pub async fn join_team(&self, server: &CloudServer) -> Result<JoinOutcome, JoinError> {
    let address = ServerAddress::parse(&server.address)
      .map_err(|error| JoinError::Server(SharingError::Client(ClientError::Address(error))))?;
    self.join_server(&address, Some(server.id)).await
  }

  /// Joins the team server at `address` with the account; `linked` is its id at the cloud, as it names it at
  /// `GET /server` ([`Cloud::join_server`]).
  pub async fn join_server(
    &self,
    address: &ServerAddress,
    linked: Option<TeamServerId>,
  ) -> Result<JoinOutcome, JoinError> {
    match self.cloud.join_server(address, linked).await {
      Ok(ServerJoin::Joined(joined)) => self.outcome(joined).await,
      Ok(ServerJoin::NotInvited) | Err(CloudError::NoServer) => Ok(JoinOutcome::NotInvited),
      Err(error) => Err(refusal::from_cloud(error, Some(address))),
    }
  }

  /// Accepts `invite` and joins its server ([`Cloud::accept`]).
  pub async fn join_invite(&self, invite: &CloudInvite) -> Result<JoinOutcome, JoinError> {
    match self.cloud.accept(invite).await {
      Ok(accepted) => self.outcome(accepted.joined).await,
      Err(error) => {
        let address = ServerAddress::parse(&invite.server.address).ok();
        Err(refusal::from_cloud(error, address.as_ref()))
      }
    }
  }

  /// Joins the team server at `address`, trusted as `trust`, with a code. Signed in to Orchester Cloud, the code goes
  /// with this computer's cloud identity and the server into the account's list ([`Cloud::join_with_code`]); without
  /// a Cloud sign-in it goes alone, and the server into the list of no account (row 45).
  pub async fn join_by_code(
    &self,
    address: &ServerAddress,
    trust: Trust,
    request: CodeRequest,
  ) -> Result<JoinOutcome, JoinError> {
    let cloud = self.cloud.clone();
    let signed_in = background::run(move || cloud.account())
      .await
      .map_err(CloudError::from)??
      .is_some();
    let server = if signed_in {
      self
        .cloud
        .join_with_code(address, trust, request)
        .await
        .map_err(|error| refusal::from_cloud(error, Some(address)))?
    } else {
      self
        .servers
        .join_with_code(address, trust, request)
        .await
        .map_err(|error| refusal::from_sharing(error, address))?
    };
    Ok(JoinOutcome::Joined(self.joined(server).await?))
  }

  /// What answers at `address` before anything is sent there ([`Servers::reach`]).
  pub async fn reach(&self, address: &ServerAddress, plain_http: PlainHttp) -> Result<Reached, JoinError> {
    self
      .servers
      .reach(address, plain_http)
      .await
      .map_err(|error| refusal::from_client(error, address))
  }

  /// The shared projects on `server` the person is a member of, each with its remote hint and the checkout linked to
  /// it here; [`JoinRefusal::NoProjects`] when there is none.
  pub async fn projects(&self, server: ServerId) -> Result<Vec<ServerProject>, JoinError> {
    let entry = self.entry(server).await?;
    let projects = self
      .servers
      .server_projects(&entry)
      .await
      .map_err(|error| refusal::from_sharing(error, &entry.address))?;
    if projects.is_empty() {
      return Err(JoinError::Refused(JoinRefusal::NoProjects {
        server: entry.stored.name,
      }));
    }
    Ok(projects)
  }

  async fn outcome(&self, joined: CloudJoined) -> Result<JoinOutcome, JoinError> {
    match joined {
      CloudJoined::Joined(server) => Ok(JoinOutcome::Joined(self.joined(server).await?)),
      CloudJoined::Waiting(approval) => Ok(JoinOutcome::Waiting(approval)),
    }
  }

  async fn joined(&self, server: ServerId) -> Result<JoinedServer, JoinError> {
    let entry = self.entry(server).await?;
    Ok(JoinedServer {
      id: server,
      name: entry.stored.name,
      // A join always keeps the account it signed in to.
      username: entry.stored.account.map(|account| account.username).unwrap_or_default(),
    })
  }

  /// `server` among the signed-in account's servers.
  async fn entry(&self, server: ServerId) -> Result<ServerEntry, SharingError> {
    self
      .entries()
      .await?
      .into_iter()
      .find(|entry| entry.stored.id == server)
      .ok_or(SharingError::ServerMissing)
  }

  async fn entries(&self) -> Result<Vec<ServerEntry>, SharingError> {
    let servers = self.servers.clone();
    background::run(move || servers.entries()).await?
  }
}
