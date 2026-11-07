use thiserror::Error;

use crate::{
  connection::AgentProvider,
  id::ResourceId,
  resource::{TeamAgentProfile, TeamMcpLaunch, TeamMcpServer},
  secret::SecretName,
};

/// One thing a resource version asks of the computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Need {
  Connection { profile: ResourceId, cli: AgentProvider },
  McpOn(ResourceId),
  McpProgram(ResourceId),
  Secret(SecretName),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NeedsError {
  /// A profile chooses an MCP server whose version was not given, so what it needs cannot be known; it is not
  /// assumed to need nothing.
  #[error("the profile chooses the MCP server {0}, whose version was not given")]
  McpServerNotGiven(ResourceId),
}

/// What one pinned version of a team MCP server needs: to be on here, its program's path when it runs a program, and
/// the secrets it is given values for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpNeed {
  server: ResourceId,
  runs_program: bool,
  secrets: Vec<SecretName>,
}

impl McpNeed {
  /// The need of `body`, the version of team MCP server `server`: the secrets are those its variables and secret
  /// headers name.
  pub fn of(server: ResourceId, body: &TeamMcpServer) -> Self {
    Self {
      server,
      runs_program: matches!(body.launch(), TeamMcpLaunch::Program(_)),
      secrets: body.secrets(),
    }
  }
}

/// Everything the versions of one resource (or of every step of a pipeline) ask of this computer, in the order they
/// ask it. [`super::readiness`] answers it against [`super::Here`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Needs(Vec<Need>);

impl Needs {
  /// Nothing: an instruction or a skill, which is always ready.
  pub fn none() -> Self {
    Self::default()
  }

  /// A team profile `profile`: a connection for its CLI kind, the MCP servers it chooses (`servers` holds the pinned
  /// version of each) and the secrets it names. Fails when it chooses a server `servers` lacks.
  pub fn profile(profile: ResourceId, body: &TeamAgentProfile, servers: &[McpNeed]) -> Result<Self, NeedsError> {
    let mut needs = vec![Need::Connection {
      profile,
      cli: body.cli(),
    }];
    for chosen in body.mcp_servers() {
      let server = servers
        .iter()
        .find(|server| server.server == *chosen)
        .ok_or(NeedsError::McpServerNotGiven(*chosen))?;
      needs.extend(Self::mcp_server(server).0);
    }
    needs.extend(body.secrets().iter().cloned().map(Need::Secret));
    Ok(Self(needs))
  }

  /// A team MCP server on its own.
  pub fn mcp_server(server: &McpNeed) -> Self {
    let program = server.runs_program.then_some(Need::McpProgram(server.server));
    let secrets = server.secrets.iter().cloned().map(Need::Secret);
    Self(
      std::iter::once(Need::McpOn(server.server))
        .chain(program)
        .chain(secrets)
        .collect(),
    )
  }

  /// A tool: only the secret values it is given. A tool body does not name its secrets by name yet, so the caller
  /// gives them.
  pub fn tool(secrets: Vec<SecretName>) -> Self {
    Self(secrets.into_iter().map(Need::Secret).collect())
  }

  /// The union of `parts`, for a pipeline: the needs of the profile or tool of each step and of the Deliver step's
  /// integrator, in step order. What several parts share is asked once.
  pub fn union(parts: impl IntoIterator<Item = Self>) -> Self {
    Self(parts.into_iter().flat_map(|part| part.0).collect())
  }

  pub(super) fn iter(&self) -> impl Iterator<Item = &Need> {
    self.0.iter()
  }
}
