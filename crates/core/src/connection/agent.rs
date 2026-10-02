//! Agent provider connections: the only kind an agent profile can run on.

use std::{ops::Deref, path::PathBuf};

use super::{
  AgentEndpoint, AgentProvider, ApiEndpoint, Connection, ConnectionError, ConnectionKind, ConnectionStatus, LastCheck,
  SignIn,
  catalog::{CatalogModel, ModelCatalog},
  compare_versions,
  model_requirements::CliTooOld,
};
use crate::{id::ConnectionId, name::Name, resource::ModelId, secret::Secret, time::Timestamp};

/// What running an agent CLI's read-only commands found, or what testing a native connection's model
/// API found (`docs/architecture/native-agent.md` §7: listing its models spends no tokens).
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
  /// The model API answered and accepted the key; `balance` as the provider reports it, where it does.
  Api { balance: Option<String> },
  /// The model API refused the key.
  KeyRejected,
  /// The model API accepted the key but does not serve it for billing; `balance` as reported.
  BillingProblem { balance: Option<String> },
  /// The model API could not be reached or did not answer in time.
  EndpointUnreachable,
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
      Self::Api { .. } => ConnectionStatus::Connected,
      Self::KeyRejected => ConnectionStatus::KeyRejected,
      Self::BillingProblem { .. } => ConnectionStatus::BillingProblem,
      Self::EndpointUnreachable => ConnectionStatus::EndpointUnreachable,
    }
  }

  /// Whether this is a check of a model API rather than of a program.
  pub const fn is_api(&self) -> bool {
    matches!(
      self,
      Self::Api { .. } | Self::KeyRejected | Self::BillingProblem { .. } | Self::EndpointUnreachable
    )
  }

  /// The balance the model API reported.
  pub fn balance(&self) -> Option<&str> {
    match self {
      Self::Api { balance } | Self::BillingProblem { balance } => balance.as_deref(),
      _ => None,
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
  /// A connection to the agent CLI at `executable` that has not been checked yet.
  pub fn new(
    id: ConnectionId,
    name: Name,
    provider: AgentProvider,
    executable: PathBuf,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    let endpoint = AgentEndpoint::Program(executable);
    let connection = Connection::new(id, name, ConnectionKind::Agent(provider), endpoint, created_at)?;
    Ok(Self { connection, provider })
  }

  /// A connection to a native `provider` at the model API `api`.
  pub fn native(
    id: ConnectionId,
    name: Name,
    provider: AgentProvider,
    api: ApiEndpoint,
    created_at: Timestamp,
  ) -> Result<Self, ConnectionError> {
    let endpoint = AgentEndpoint::Api(api);
    let connection = Connection::new(id, name, ConnectionKind::Agent(provider), endpoint, created_at)?;
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

  /// The model API of a native connection.
  pub const fn api(&self) -> Option<&ApiEndpoint> {
    self.connection.endpoint.api()
  }

  /// See [`Connection::set_api`].
  pub fn set_api(&mut self, api: ApiEndpoint) -> Result<(), ConnectionError> {
    self.connection.set_api(api)
  }

  /// The connection's status where `active` are the project's secrets that are not archived: a native
  /// connection whose key secret is not among them reads *Key missing*, whatever its last check found,
  /// since nothing reaches the API without the key.
  pub fn status_with(&self, active: &[Secret]) -> ConnectionStatus {
    match self.api() {
      Some(api)
        if !active
          .iter()
          .any(|secret| secret.id() == api.key() && !secret.is_archived()) =>
      {
        ConnectionStatus::KeyMissing
      }
      _ => self.connection.status(),
    }
  }

  pub fn last_check(&self) -> Option<&ConnectionCheck> {
    match self.connection.last_check.as_ref()? {
      LastCheck::Agent(check) => Some(check),
      LastCheck::GitHub(_) => None,
    }
  }

  /// See [`Connection::record_check`]: a native connection takes a check of its model API only.
  pub fn record_check(&mut self, check: ConnectionCheck) -> Result<(), ConnectionError> {
    self.connection.record_check(LastCheck::Agent(check))
  }

  /// Models found by the last successful check.
  pub fn catalog(&self) -> Option<&ModelCatalog> {
    match &self.last_check()?.result {
      CheckResult::Available { catalog, .. } => Some(catalog),
      _ => None,
    }
  }

  /// Records what a refusal said: `model` needs version `required` of this CLI or newer.
  pub fn record_model_requirement(&mut self, model: &ModelId, required: &str) {
    self.connection.model_requirements.record(model, required);
  }

  /// Why the checked CLI cannot run `model`: its version is below the one a refusal recorded.
  /// `None` when nothing was recorded, the CLI is new enough, or its version is unknown.
  pub fn cannot_run(&self, model: &ModelId) -> Option<CliTooOld> {
    let required = self.connection.model_requirements.required(model)?;
    let CheckResult::Available { version, .. } = &self.last_check()?.result else {
      return None;
    };
    compare_versions(Some(version), Some(required))
      .is_lt()
      .then(|| CliTooOld {
        installed: version.clone(),
        required: required.to_owned(),
      })
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
