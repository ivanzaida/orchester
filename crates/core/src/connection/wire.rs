//! The serialized form of a [`Connection`], in which the executor's interfaces answer
//! (`docs/architecture/remote-runners.md`, rule 4). It is read back through the checks a stored connection is read
//! through: an endpoint that does not fit the kind, a check of another kind or an account the connection cannot run
//! on is refused.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
  AgentEndpoint, AgentProvider, ApiEndpoint, ApiProvider, CheckResult, Connection, ConnectionAccount, ConnectionCheck,
  ConnectionError, ConnectionKind, GitHubCheck, GitHubCheckResult, GitHubHost, LastCheck, ModelRequirements,
};
use crate::{id::ConnectionId, name::Name, time::Timestamp};

#[derive(Serialize, Deserialize)]
pub(super) struct ConnectionWire {
  id: ConnectionId,
  name: Name,
  kind: KindWire,
  endpoint: EndpointWire,
  last_check: Option<CheckWire>,
  created_at: Timestamp,
  #[serde(default, skip_serializing_if = "ModelRequirements::is_empty")]
  model_requirements: ModelRequirements,
  #[serde(default, skip_serializing_if = "ConnectionAccount::is_default")]
  account: ConnectionAccount,
  api_provider: Option<ApiProvider>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum KindWire {
  Agent { provider: AgentProvider },
  GitHub { host: GitHubHost },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EndpointWire {
  Program(PathBuf),
  Api(ApiEndpoint),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CheckWire {
  Agent { at: Timestamp, result: CheckResult },
  GitHub { at: Timestamp, result: GitHubCheckResult },
}

impl From<Connection> for ConnectionWire {
  fn from(connection: Connection) -> Self {
    Self {
      id: connection.id,
      name: connection.name,
      kind: match connection.kind {
        ConnectionKind::Agent(provider) => KindWire::Agent { provider },
        ConnectionKind::GitHub { host } => KindWire::GitHub { host },
      },
      endpoint: match connection.endpoint {
        AgentEndpoint::Program(program) => EndpointWire::Program(program),
        AgentEndpoint::Api(api) => EndpointWire::Api(api),
      },
      last_check: connection.last_check.map(|check| match check {
        LastCheck::Agent(ConnectionCheck { at, result }) => CheckWire::Agent { at, result },
        LastCheck::GitHub(GitHubCheck { at, result }) => CheckWire::GitHub { at, result },
      }),
      created_at: connection.created_at,
      model_requirements: connection.model_requirements,
      account: connection.account,
      api_provider: connection.api_provider,
    }
  }
}

impl TryFrom<ConnectionWire> for Connection {
  type Error = ConnectionError;

  fn try_from(wire: ConnectionWire) -> Result<Self, ConnectionError> {
    let kind = match wire.kind {
      KindWire::Agent { provider } => ConnectionKind::Agent(provider),
      KindWire::GitHub { host } => ConnectionKind::GitHub { host },
    };
    let endpoint = match wire.endpoint {
      EndpointWire::Program(program) => AgentEndpoint::Program(program),
      EndpointWire::Api(api) => AgentEndpoint::Api(api),
    };
    let last_check = wire.last_check.map(|check| match check {
      CheckWire::Agent { at, result } => LastCheck::Agent(ConnectionCheck { at, result }),
      CheckWire::GitHub { at, result } => LastCheck::GitHub(GitHubCheck { at, result }),
    });
    Self::from_stored(wire.id, wire.name, kind, endpoint, last_check, wire.created_at)?
      .with_model_requirements(wire.model_requirements)
      .with_account(wire.account)?
      .with_api_provider(wire.api_provider)
  }
}
