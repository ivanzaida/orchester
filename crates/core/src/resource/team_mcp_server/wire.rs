//! The JSON of a team MCP server. Unknown fields are refused, so `path`, `value` and `id` have nowhere to go.

use serde::{Deserialize, Serialize};

use super::{TeamMcpLaunch, TeamMcpProgram, TeamMcpRemote, TeamMcpServer, TeamMcpServerError};
use crate::{
  connection::mcp::{ExemptTools, McpHeaderName, McpServerName, McpUrl, WorkingDirectory},
  resource::{CommandArg, CommandProgram, VariableName},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeamMcpServerWire {
  name: McpServerName,
  launch: LaunchWire,
  #[serde(default, skip_serializing_if = "ExemptTools::is_empty")]
  exempt_tools: ExemptTools,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum LaunchWire {
  Program {
    program: CommandProgram,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<CommandArg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    working_directory: Option<WorkingDirectory>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variables: Vec<VariableName>,
  },
  Remote {
    url: McpUrl,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    secret_headers: Vec<McpHeaderName>,
  },
}

impl TryFrom<TeamMcpServerWire> for TeamMcpServer {
  type Error = TeamMcpServerError;

  fn try_from(wire: TeamMcpServerWire) -> Result<Self, TeamMcpServerError> {
    let launch = match wire.launch {
      LaunchWire::Program {
        program,
        args,
        working_directory,
        variables,
      } => TeamMcpLaunch::Program(TeamMcpProgram::new(program, args, working_directory, variables)?),
      LaunchWire::Remote { url, secret_headers } => TeamMcpLaunch::Remote(TeamMcpRemote::new(url, secret_headers)?),
    };
    Ok(Self::new(wire.name, launch, wire.exempt_tools))
  }
}

impl From<TeamMcpServer> for TeamMcpServerWire {
  fn from(server: TeamMcpServer) -> Self {
    let launch = match server.launch {
      TeamMcpLaunch::Program(program) => LaunchWire::Program {
        program: program.program,
        args: program.args,
        working_directory: program.working_directory,
        variables: program.variables,
      },
      TeamMcpLaunch::Remote(remote) => LaunchWire::Remote {
        url: remote.url,
        secret_headers: remote.secret_headers,
      },
    };
    Self {
      name: server.name,
      launch,
      exempt_tools: server.exempt_tools,
    }
  }
}
