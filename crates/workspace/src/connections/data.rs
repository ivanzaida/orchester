//! Reading, adding and checking project connections through the project database. Every
//! function blocks on SQLite or on the CLI being checked; the UI calls them off the UI thread.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, Connection, ConnectionCheck, ConnectionError, GitHubConnection, mcp::McpServer,
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  repository::RepositoryLink,
  resource::ModelId,
  secret::Secret,
  store::{ConnectionStore, McpServerStore, RepositoryStore, SecretStore},
  time::Timestamp,
};
use orchester_exec::launch::{self, Launch};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{
  model_uses::{self, ModelUse},
  probe,
};
use crate::{
  agents::data::profiles,
  messages::{self, Message},
};

/// A profile that runs on a connection, or gives its agents an MCP server, for the read-only "Used by"
/// links.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileUse {
  pub connection: ConnectionId,
  pub profile: ResourceId,
  pub role: Arc<str>,
  pub model: ModelId,
}

/// The project's connections of each kind, what starts when each one's program runs, the
/// profiles that use the agent providers and the MCP servers, the repository link that uses a GitHub
/// connection and the secrets MCP servers can take values from, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectConnections {
  pub connections: Arc<Vec<AgentConnection>>,
  pub github: Arc<Vec<GitHubConnection>>,
  pub mcp: Arc<Vec<McpServer>>,
  /// The project's secrets that are not archived.
  pub secrets: Arc<Vec<Secret>>,
  pub uses: Arc<Vec<ProfileUse>>,
  /// Who runs on which model of each agent connection.
  pub model_uses: Arc<Vec<ModelUse>>,
  pub link: Option<Arc<RepositoryLink>>,
  pub launches: Arc<Vec<(ConnectionId, Launch)>>,
}

impl ProjectConnections {
  pub fn load(store: &ProjectDb) -> Result<Self, StoreError> {
    // A profile uses the connection and the MCP servers its current content names: its draft if it
    // has one.
    let uses = profiles(store)?
      .into_iter()
      .flat_map(|entry| {
        let servers = entry.profile.mcp_servers().ids().to_vec();
        std::iter::once(entry.profile.connection())
          .chain(servers)
          .map(move |connection| ProfileUse {
            connection,
            profile: entry.id,
            role: entry.profile.role().as_str().into(),
            model: entry.profile.model().clone(),
          })
      })
      .collect();
    let connections = store.connections()?;
    let launches = connections
      .iter()
      .filter_map(|connection| Some((connection.id(), launch::resolve(connection.executable()?))))
      .collect();
    for server in store.unreadable_mcp_servers()? {
      tracing::warn!(
        server = %server.id,
        name = %server.name,
        detail = %server.detail,
        "an MCP server cannot be read and is left out of the connections"
      );
    }
    Ok(Self {
      github: Arc::new(github_connections(connections.clone())),
      connections: Arc::new(agent_connections(connections)),
      mcp: Arc::new(store.mcp_servers()?),
      secrets: Arc::new(store.secrets()?),
      uses: Arc::new(uses),
      model_uses: Arc::new(model_uses::load(store)?),
      link: store.repository_link()?.map(Arc::new),
      launches: Arc::new(launches),
    })
  }

  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    Self::load(&ProjectDb::open(repo_root)?)
  }

  pub fn connection(&self, id: ConnectionId) -> Option<&AgentConnection> {
    self.connections.iter().find(|connection| connection.id() == id)
  }

  pub fn github_connection(&self, id: ConnectionId) -> Option<&GitHubConnection> {
    self.github.iter().find(|connection| connection.id() == id)
  }

  pub fn mcp_server(&self, id: ConnectionId) -> Option<&McpServer> {
    self.mcp.iter().find(|server| server.id() == id)
  }

  /// What starts when the program of connection `id` runs.
  pub fn launch_of(&self, id: ConnectionId) -> Option<&Launch> {
    self
      .launches
      .iter()
      .find(|(connection, _)| *connection == id)
      .map(|(_, launch)| launch)
  }

  /// The repository link reached through `id`, if the project's link uses that connection.
  pub fn link_of(&self, id: ConnectionId) -> Option<&RepositoryLink> {
    self.link.as_deref().filter(|link| link.connection() == id)
  }

  /// Profiles that run on `id`, or give their agents MCP server `id`, in profile order.
  pub fn used_by(&self, id: ConnectionId) -> impl Iterator<Item = &ProfileUse> {
    self.uses.iter().filter(move |used| used.connection == id)
  }
}

/// The agent provider connections among `connections`, in their order.
pub fn agent_connections(connections: Vec<Connection>) -> Vec<AgentConnection> {
  connections
    .into_iter()
    // Other kinds are not agent providers; dropping the conversion error is how they are
    // filtered out.
    .filter_map(|connection| AgentConnection::try_from(connection).ok())
    .collect()
}

/// The GitHub connections among `connections`, in their order.
pub fn github_connections(connections: Vec<Connection>) -> Vec<GitHubConnection> {
  connections
    .into_iter()
    // Other kinds are agent providers; dropping the conversion error filters them out.
    .filter_map(|connection| GitHubConnection::try_from(connection).ok())
    .collect()
}

/// A connection as the add or edit dialog saves it, with the check the dialog ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionDraft {
  /// The connection being edited; `None` adds a new one.
  pub id: Option<ConnectionId>,
  pub name: Name,
  pub provider: AgentProvider,
  pub executable: PathBuf,
  pub check: ConnectionCheck,
}

#[derive(Debug, Error)]
pub enum SaveError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Connection(#[from] ConnectionError),
  /// The check did not find the expected CLI; the dialog does not offer saving then.
  #[error("the connection was not verified")]
  NotVerified,
  /// A native connection is not checked in this version.
  #[error("native agents are not available in this version")]
  NativeNotAvailable,
}

impl SaveError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::Connection(ConnectionError::RelativeExecutable(_)) => Message::new("errors.executableNotAbsolute"),
      // The dialog edits agent connections only; a stored connection of another kind under
      // the edited id means the stored data does not match what was shown.
      Self::Connection(
        ConnectionError::NotAgent(_)
        | ConnectionError::NotGitHub(_)
        | ConnectionError::CheckKindMismatch(_)
        | ConnectionError::PresetNotOffered
        | ConnectionError::FallbackNotOffered,
      ) => Message::new("errors.storage"),
      // Only a native connection has no program to point at.
      Self::Connection(ConnectionError::EndpointMismatch(_)) | Self::NativeNotAvailable => {
        Message::new("errors.nativeNotAvailable")
      }
      Self::NotVerified => Message::new("errors.connectionNotVerified"),
    }
  }
}

/// Adds or updates the connection in `draft` with its check.
pub fn save<S>(store: &mut S, draft: ConnectionDraft, now: Timestamp) -> Result<ConnectionId, SaveError>
where
  S: ConnectionStore<Error = StoreError>,
{
  if !draft.check.result.is_verified() {
    return Err(SaveError::NotVerified);
  }
  let Some(id) = draft.id else {
    let mut connection = AgentConnection::new(
      ConnectionId::generate(),
      draft.name,
      draft.provider,
      draft.executable,
      now,
    )?;
    connection.record_check(draft.check)?;
    store.create_connection(&connection)?;
    return Ok(connection.id());
  };
  let stored = store.connection(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  let mut connection = AgentConnection::try_from(stored)?;
  connection.rename(draft.name);
  connection.set_executable(draft.executable)?;
  connection.record_check(draft.check)?;
  store.save_connection(&connection)?;
  Ok(id)
}

pub fn save_in(repo_root: &Path, draft: ConnectionDraft) -> Result<ConnectionId, SaveError> {
  save(&mut ProjectDb::open(repo_root)?, draft, Timestamp::now())
}

/// Checks a stored connection's CLI again and stores the result, whatever it is. A native
/// connection is not checked in this version.
pub fn recheck_in(repo_root: &Path, id: ConnectionId) -> Result<(), SaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let mut connection = db.connection(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  let check = probe::recheck(&connection).ok_or(SaveError::NativeNotAvailable)?;
  connection
    .record_check(check)
    .expect("the check was made for the connection's own kind");
  db.save_connection(&connection)?;
  Ok(())
}
