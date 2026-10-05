//! A shared project on the desktop (`docs/architecture/server.md`, phase S2b): the server project a
//! project's task list lives in, reached through the server it is on. The project database is the
//! cache; every change goes to the server first and the cache takes what the server confirmed
//! ([`store::RemoteTaskStore`]). While the server cannot be reached, or the person is no longer a
//! member, nothing is sent: the change fails at once and the list stays read-only.

mod automatic;
mod claims;
pub mod convert;
pub mod discovery;
pub mod lookup;
mod make_local;
mod members;
mod organization;
mod share;
pub mod state;
pub mod store;
mod sync;
pub mod work;

use std::path::{Path, PathBuf};

use orchester_api::client::{Client, ClientError};
use orchester_core::{
  id::{ProjectId, UserId},
  secret::SecretValue,
};
use orchester_store::{ProjectDb, SharingLink};

pub use self::{
  claims::WorkRefusal,
  members::MemberChange,
  share::{IdentityProblem, ShareError, SharePlan, Shared, plan, repository_identity},
};
use super::{Connection, ServerEntry, Servers, SharingError};

/// A shared project's server project and the server it is on.
#[derive(Clone)]
pub struct ProjectServer {
  servers: Servers,
  entry: ServerEntry,
  link: SharingLink,
  repo_root: PathBuf,
}

impl ProjectServer {
  /// The server project of the project at `repo_root`; `None` while the project is local. Blocking.
  pub fn open(servers: &Servers, repo_root: &Path) -> Result<Option<Self>, SharingError> {
    let db = ProjectDb::open(repo_root)?;
    let Some(link) = db.sharing()? else {
      return Ok(None);
    };
    let entry = servers
      .entries()?
      .into_iter()
      .find(|entry| entry.stored.id == link.server)
      .ok_or(SharingError::ServerMissing)?;
    Ok(Some(Self {
      servers: servers.clone(),
      entry,
      link,
      repo_root: repo_root.to_owned(),
    }))
  }

  pub fn link(&self) -> &SharingLink {
    &self.link
  }

  pub fn entry(&self) -> &ServerEntry {
    &self.entry
  }

  pub fn repo_root(&self) -> &Path {
    &self.repo_root
  }

  /// The person signed in to the server from this computer.
  pub fn me(&self) -> Option<UserId> {
    self.entry.stored.account.as_ref().map(|account| account.user)
  }

  /// Fails at once while the server cannot take a change: it cannot be reached, nobody is signed
  /// in to it, or the person is no longer a member.
  pub fn writable(&self) -> Result<(), SharingError> {
    if self.link.removed_at.is_some() {
      return Err(SharingError::NotMember);
    }
    if self.entry.stored.account.is_none() {
      return Err(SharingError::SignedOut);
    }
    reachable(self.servers.connection(self.link.server))
  }

  /// Runs `call` with the server's client, the person's token and the server project's id.
  async fn call<T, F, Fut>(&self, call: F) -> Result<T, SharingError>
  where
    F: FnOnce(Client, SecretValue, ProjectId) -> Fut,
    Fut: Future<Output = Result<T, ClientError>>,
  {
    let project = self.link.project;
    self
      .servers
      .with_token(&self.entry, |client, token| call(client, token, project))
      .await
  }
}

/// Whether a server in `connection` takes changes now; decision 16: reconnecting and offline both
/// make the list read-only.
fn reachable(connection: Connection) -> Result<(), SharingError> {
  match connection {
    Connection::Connected | Connection::Checking => Ok(()),
    Connection::Reconnecting { .. } | Connection::Offline { .. } => Err(SharingError::Offline),
    Connection::SignedOut => Err(SharingError::SignedOut),
  }
}
