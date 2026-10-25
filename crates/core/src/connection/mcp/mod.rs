//! MCP servers of a project (`docs/design/orchester-workspace.md`, "MCP servers"): connections an
//! agent profile can give its agents. Agents run with the user's own CLI configuration left out
//! (`--ignore-user-config`, `--strict-mcp-config`), so the servers a project configures here are the
//! only ones they get. The agent's CLI starts a server with the agent and ends it with the agent;
//! Orchester itself never talks to a server.
//!
//! A server has a name, a transport, variables for a stdio server's program whose values may come
//! from project secrets, the tools that run without asking ([`ExemptTools`]), and an enabled flag. Its id is a
//! [`ConnectionId`]: the Connections screen lists servers beside the other connections.

mod authorization;
mod environment;
mod exempt;
mod name;
mod problem;
mod transport;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  authorization::{BasicUsername, McpAuthorization, McpAuthorizationError, McpHeaderName},
  environment::{McpEnvValue, McpEnvironment, McpEnvironmentError},
  exempt::{ExemptTools, ExemptToolsError, InvocationExemptions, McpToolName, McpToolNameError, UnappliedExemption},
  name::{McpServerName, McpServerNameError},
  problem::McpServerProblem,
  transport::{HttpServer, McpTransport, McpUrl, StdioServer, TransportError, WorkingDirectory},
};
use crate::{
  id::{ConnectionId, SecretId},
  time::Timestamp,
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum McpServerError {
  /// Variables are set on a program the CLI starts; an HTTP server is not started.
  #[error("only a server the agent's CLI starts has environment variables")]
  EnvironmentOnHttp,
}

/// A stored MCP server that cannot be read back, for example because its URL breaks a rule added
/// after it was saved: listings leave it out, so it never hides the other servers, and
/// [`crate::store::McpServerStore::unreadable_mcp_servers`] reports it. The store keeps the row
/// unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnreadableMcpServer {
  /// The id and name as stored; either may be what cannot be read.
  pub id: String,
  pub name: String,
  /// What is wrong with the row.
  pub detail: String,
}

/// A project's MCP server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpServer {
  id: ConnectionId,
  name: McpServerName,
  transport: McpTransport,
  environment: McpEnvironment,
  exempt_tools: ExemptTools,
  enabled: bool,
  created_at: Timestamp,
}

impl McpServer {
  /// An enabled server.
  pub fn new(
    id: ConnectionId,
    name: McpServerName,
    transport: McpTransport,
    environment: McpEnvironment,
    created_at: Timestamp,
  ) -> Result<Self, McpServerError> {
    check(&transport, &environment)?;
    Ok(Self {
      id,
      name,
      transport,
      environment,
      exempt_tools: ExemptTools::default(),
      enabled: true,
      created_at,
    })
  }

  pub fn from_stored(
    id: ConnectionId,
    name: McpServerName,
    (transport, environment): (McpTransport, McpEnvironment),
    enabled: bool,
    created_at: Timestamp,
  ) -> Result<Self, McpServerError> {
    let mut server = Self::new(id, name, transport, environment, created_at)?;
    server.enabled = enabled;
    Ok(server)
  }

  pub const fn id(&self) -> ConnectionId {
    self.id
  }

  pub const fn name(&self) -> &McpServerName {
    &self.name
  }

  pub const fn transport(&self) -> &McpTransport {
    &self.transport
  }

  pub const fn environment(&self) -> &McpEnvironment {
    &self.environment
  }

  /// The tools that run without asking when a profile asks before MCP actions.
  pub const fn exempt_tools(&self) -> &ExemptTools {
    &self.exempt_tools
  }

  /// A disabled server is given to no agent, whichever profiles choose it.
  pub const fn enabled(&self) -> bool {
    self.enabled
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  pub const fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
  }

  pub fn set_exempt_tools(&mut self, tools: ExemptTools) {
    self.exempt_tools = tools;
  }

  /// Replaces the name, transport and variables together, as the edit dialog saves them.
  pub fn update(
    &mut self,
    name: McpServerName,
    transport: McpTransport,
    environment: McpEnvironment,
  ) -> Result<(), McpServerError> {
    check(&transport, &environment)?;
    self.name = name;
    self.transport = transport;
    self.environment = environment;
    Ok(())
  }

  /// The project secrets the server uses: the one its authorization sends or its variables' values, each once.
  pub fn secrets(&self) -> Vec<SecretId> {
    match &self.transport {
      McpTransport::Http(http) => http.authorization().secret().into_iter().collect(),
      McpTransport::Stdio(_) => self.environment.secrets().collect(),
    }
  }
}

fn check(transport: &McpTransport, environment: &McpEnvironment) -> Result<(), McpServerError> {
  if matches!(transport, McpTransport::Http(_)) && !environment.is_empty() {
    return Err(McpServerError::EnvironmentOnHttp);
  }
  Ok(())
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum McpSelectionError {
  #[error("MCP server {0} is chosen twice")]
  Duplicate(ConnectionId),
  #[error("a profile chooses at most {max} MCP servers", max = McpServerSelection::MAX)]
  TooMany,
}

/// The MCP servers an agent profile gives its agents, in the order they were chosen. None by
/// default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Vec<ConnectionId>", into = "Vec<ConnectionId>")]
pub struct McpServerSelection(Vec<ConnectionId>);

impl McpServerSelection {
  pub const MAX: usize = 20;

  /// No servers.
  pub const fn none() -> Self {
    Self(Vec::new())
  }

  pub fn new(servers: Vec<ConnectionId>) -> Result<Self, McpSelectionError> {
    if servers.len() > Self::MAX {
      return Err(McpSelectionError::TooMany);
    }
    for (index, server) in servers.iter().enumerate() {
      if servers[..index].contains(server) {
        return Err(McpSelectionError::Duplicate(*server));
      }
    }
    Ok(Self(servers))
  }

  pub fn ids(&self) -> &[ConnectionId] {
    &self.0
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn contains(&self, server: ConnectionId) -> bool {
    self.0.contains(&server)
  }

  /// The servers of `servers` this selection gives an agent (chosen and on) that list tools to run
  /// without asking, in `servers`' order, with those tools.
  pub fn exempt_tools<'a>(&self, servers: &'a [McpServer]) -> Vec<(&'a McpServerName, &'a ExemptTools)> {
    servers
      .iter()
      .filter(|server| server.enabled() && self.contains(server.id()) && !server.exempt_tools().is_empty())
      .map(|server| (server.name(), server.exempt_tools()))
      .collect()
  }

  /// This selection with `server` chosen or not.
  pub fn toggled(&self, server: ConnectionId) -> Result<Self, McpSelectionError> {
    let mut servers = self.0.clone();
    match servers.iter().position(|chosen| *chosen == server) {
      Some(index) => {
        servers.remove(index);
      }
      None => servers.push(server),
    }
    Self::new(servers)
  }
}

impl TryFrom<Vec<ConnectionId>> for McpServerSelection {
  type Error = McpSelectionError;

  fn try_from(servers: Vec<ConnectionId>) -> Result<Self, McpSelectionError> {
    Self::new(servers)
  }
}

impl From<McpServerSelection> for Vec<ConnectionId> {
  fn from(selection: McpServerSelection) -> Self {
    selection.0
  }
}
