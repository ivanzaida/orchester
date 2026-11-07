use thiserror::Error;

use super::McpUnsupported;
use crate::{
  connection::{AgentProvider, mcp::McpServerName},
  id::{ConnectionId, ResourceId, SecretId},
  resource::{TeamAgentProfile, TeamMcpLaunch, TeamMcpServer},
  secret::SecretName,
};

/// One thing a resource version asks of the computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Need {
  Connection { profile: ResourceId, cli: AgentProvider },
  McpOn(ResourceId),
  McpUnsupported { server: ResourceId, why: McpUnsupported },
  McpProgram(ResourceId),
  Secret(SecretName),
  LocalMcpProgram { server: ConnectionId, name: McpServerName },
  LocalSecret { secret: SecretId, name: SecretName },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NeedsError {
  /// A profile chooses an MCP server whose version was not given, so what it needs cannot be known; it is not
  /// assumed to need nothing.
  #[error("the profile chooses the MCP server {0}, whose version was not given")]
  McpServerNotGiven(ResourceId),
}

/// What one pinned version of a team MCP server needs: to be on here, its program's path when it runs a program, and
/// the secrets it is given values for; and, when this app cannot run it as defined, why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpNeed {
  server: ResourceId,
  runs_program: bool,
  unsupported: Option<McpUnsupported>,
  secrets: Vec<SecretName>,
}

impl McpNeed {
  /// The need of `body`, the version of team MCP server `server`: the secrets are those its variables and secret
  /// headers name.
  pub fn of(server: ResourceId, body: &TeamMcpServer) -> Self {
    let unsupported = match body.launch() {
      TeamMcpLaunch::Remote(remote) if remote.secret_headers().len() > 1 => {
        Some(McpUnsupported::MoreThanOneSecretHeader)
      }
      TeamMcpLaunch::Remote(_) | TeamMcpLaunch::Program(_) => None,
    };
    Self {
      server,
      runs_program: matches!(body.launch(), TeamMcpLaunch::Program(_)),
      unsupported,
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
    let mut needs = Self::pinned_profile(profile, (body.cli(), body.mcp_servers()), servers)?;
    needs.0.extend(body.secrets().iter().cloned().map(Need::Secret));
    Ok(needs)
  }

  /// A profile as an execution bundle pins it: a connection for the CLI kind the bundle binds it to and the MCP servers
  /// it names (`servers` holds the pinned version of each). The bundle's profile body names no team secret of its own.
  pub fn pinned_profile(
    profile: ResourceId,
    (cli, chosen): (AgentProvider, &[ResourceId]),
    servers: &[McpNeed],
  ) -> Result<Self, NeedsError> {
    let mut needs = vec![Need::Connection { profile, cli }];
    for chosen in chosen {
      let server = servers
        .iter()
        .find(|server| server.server == *chosen)
        .ok_or(NeedsError::McpServerNotGiven(*chosen))?;
      needs.extend(Self::mcp_server(server).0);
    }
    Ok(Self(needs))
  }

  /// A profile of a local project: a connection of the CLI kind of the one it runs on, which is its pick, and the
  /// program of each of the project's MCP servers it chooses that a run starts (`programs`, by id and name). It chooses
  /// no team MCP server and names no team secret.
  pub fn local_profile(profile: ResourceId, cli: AgentProvider, programs: &[(ConnectionId, McpServerName)]) -> Self {
    Self(
      std::iter::once(Need::Connection { profile, cli })
        .chain(programs.iter().map(|(server, name)| Need::LocalMcpProgram {
          server: *server,
          name: name.clone(),
        }))
        .collect(),
    )
  }

  /// A team MCP server on its own.
  pub fn mcp_server(server: &McpNeed) -> Self {
    let program = server.runs_program.then_some(Need::McpProgram(server.server));
    let unsupported = server.unsupported.map(|why| Need::McpUnsupported {
      server: server.server,
      why,
    });
    let secrets = server.secrets.iter().cloned().map(Need::Secret);
    Self(
      std::iter::once(Need::McpOn(server.server))
        .chain(unsupported)
        .chain(program)
        .chain(secrets)
        .collect(),
    )
  }

  /// A tool: only the team secret values it is given. A tool body does not name its secrets by name yet, so the
  /// caller gives them.
  pub fn tool(secrets: Vec<SecretName>) -> Self {
    Self(secrets.into_iter().map(Need::Secret).collect())
  }

  /// A tool of a local project that sends or connects with the project secret `secret` (`name` is what the person
  /// knows it by): the value has to be set here.
  pub fn local_tool(secret: SecretId, name: SecretName) -> Self {
    Self(vec![Need::LocalSecret { secret, name }])
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
