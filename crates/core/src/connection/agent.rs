//! Agent provider connections: the only kind an agent profile can run on.

use std::{ops::Deref, path::PathBuf};

use super::{
  AgentProvider, Connection, ConnectionError, ConnectionKind, ConnectionStatus, LastCheck, SignIn,
  catalog::{CatalogModel, ModelCatalog},
};
use crate::{id::ConnectionId, name::Name, time::Timestamp};

/// What running an agent CLI's read-only commands found.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CheckResult {
  /// The program answered as the expected CLI.
  Available {
    version: String,
    sign_in: SignIn,
    catalog: ModelCatalog,
  },
  /// Nothing to run at the executable path.
  ExecutableMissing,
  /// The program ran but did not answer as the expected CLI.
  NotRecognized,
  /// The program could not be started or did not answer in time.
  CouldNotRun,
}

impl CheckResult {
  /// The program is the expected CLI, so a connection to it may be saved. Signing in can
  /// follow; the connection then shows that it needs it.
  pub const fn is_verified(&self) -> bool {
    matches!(self, Self::Available { .. })
  }

  pub const fn status(&self) -> ConnectionStatus {
    match self {
      Self::Available { sign_in, .. } => sign_in_status(*sign_in),
      Self::ExecutableMissing => ConnectionStatus::ExecutableMissing,
      Self::NotRecognized => ConnectionStatus::NotRecognized,
      Self::CouldNotRun => ConnectionStatus::CouldNotRun,
    }
  }
}

pub(super) const fn sign_in_status(sign_in: SignIn) -> ConnectionStatus {
  match sign_in {
    SignIn::SignedIn => ConnectionStatus::Connected,
    SignIn::SignedOut => ConnectionStatus::SignInRequired,
    SignIn::Unknown => ConnectionStatus::SignInUnknown,
  }
}

/// A check of an agent CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionCheck {
  pub at: Timestamp,
  pub result: CheckResult,
}

/// A connection to an agent provider CLI. Derefs to the [`Connection`] it is; changes go
/// through its own methods so the kind cannot change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentConnection {
  connection: Connection,
  provider: AgentProvider,
}

impl AgentConnection {
  /// A connection that has not been checked yet.
  pub fn new(
    id: ConnectionId,
    name: Name,
    provider: AgentProvider,
    executable: PathBuf,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    let connection = Connection::new(id, name, ConnectionKind::Agent(provider), executable, created_at)?;
    Ok(Self { connection, provider })
  }

  pub const fn provider(&self) -> AgentProvider {
    self.provider
  }

  pub fn into_connection(self) -> Connection {
    self.connection
  }

  pub fn rename(&mut self, name: Name) {
    self.connection.rename(name);
  }

  /// See [`Connection::set_executable`].
  pub fn set_executable(&mut self, executable: PathBuf) -> Result<(), ConnectionError> {
    self.connection.set_executable(executable)
  }

  pub fn last_check(&self) -> Option<&ConnectionCheck> {
    match self.connection.last_check.as_ref()? {
      LastCheck::Agent(check) => Some(check),
      LastCheck::GitHub(_) => None,
    }
  }

  pub fn record_check(&mut self, check: ConnectionCheck) {
    self.connection.last_check = Some(LastCheck::Agent(check));
  }

  /// Models found by the last successful check.
  pub fn catalog(&self) -> Option<&ModelCatalog> {
    match &self.last_check()?.result {
      CheckResult::Available { catalog, .. } => Some(catalog),
      _ => None,
    }
  }

  /// Models a profile can choose from on this connection, in catalog order.
  pub fn models(&self) -> &[CatalogModel] {
    self.catalog().map_or(&[], ModelCatalog::models)
  }
}

impl TryFrom<Connection> for AgentConnection {
  type Error = ConnectionError;

  fn try_from(connection: Connection) -> Result<Self, ConnectionError> {
    match connection.kind {
      ConnectionKind::Agent(provider) => Ok(Self { connection, provider }),
      ConnectionKind::GitHub { .. } => Err(ConnectionError::NotAgent(connection.id)),
    }
  }
}

impl Deref for AgentConnection {
  type Target = Connection;

  fn deref(&self) -> &Connection {
    &self.connection
  }
}
