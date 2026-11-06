//! The team's MCP server: what the team server holds of one. A program is named, never located (each computer sets
//! the path of its own), and variables and secret headers are names whose values each computer fills in. The wire
//! shape refuses any other field, so a path, a value or a local id cannot be sent by mistake.
//! `docs/design/team-configuration.md`, "Who owns what".

mod wire;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{CommandArg, CommandError, CommandProgram, ProgramLocation, ResourceBody, ResourceKind, VariableName};
use crate::{
  connection::mcp::{
    ExemptTools, McpAuthorizationError, McpHeaderName, McpServerName, McpServerNameError, McpUrl, TransportError,
    WorkingDirectory,
  },
  permission::widening::CommandWidening,
  sharing::{SharedBody, VersionWidening},
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TeamMcpServerError {
  #[error(transparent)]
  Name(#[from] McpServerNameError),
  #[error(transparent)]
  Command(#[from] CommandError),
  #[error(transparent)]
  Transport(#[from] TransportError),
  #[error(transparent)]
  Header(#[from] McpAuthorizationError),
  /// The path of a program is set on each computer, so the team names it only: `npx`, not `C:\tools\npx`.
  #[error("the program `{0}` must be a name looked up on the PATH, not a path; each computer sets the path itself")]
  ProgramPath(String),
  /// A folder is relative to the worktree, which is the same on every computer.
  #[error("the working directory `{0}` must be relative to the worktree")]
  AbsoluteFolder(String),
  #[error("`{0}` is listed twice")]
  Duplicate(String),
}

/// A program the agent's CLI starts: its name, its arguments, the folder it starts in and the names of the variables
/// it needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamMcpProgram {
  program: CommandProgram,
  args: Vec<CommandArg>,
  working_directory: Option<WorkingDirectory>,
  variables: Vec<VariableName>,
}

impl TeamMcpProgram {
  pub fn new(
    program: CommandProgram,
    args: Vec<CommandArg>,
    working_directory: Option<WorkingDirectory>,
    variables: Vec<VariableName>,
  ) -> Result<Self, TeamMcpServerError> {
    if !matches!(program.location(), ProgramLocation::OnPath(_)) {
      return Err(TeamMcpServerError::ProgramPath(program.as_str().to_owned()));
    }
    if let Some(folder) = &working_directory
      && is_rooted(folder.as_str())
    {
      return Err(TeamMcpServerError::AbsoluteFolder(folder.as_str().to_owned()));
    }
    Ok(Self {
      program,
      args,
      working_directory,
      variables: distinct(variables, VariableName::as_str)?,
    })
  }

  pub const fn program(&self) -> &CommandProgram {
    &self.program
  }

  pub fn args(&self) -> &[CommandArg] {
    &self.args
  }

  pub const fn working_directory(&self) -> Option<&WorkingDirectory> {
    self.working_directory.as_ref()
  }

  /// The variables a computer gives values for.
  pub const fn variables(&self) -> &Vec<VariableName> {
    &self.variables
  }
}

/// A server already running somewhere, with the names of the headers a computer fills in with secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamMcpRemote {
  url: McpUrl,
  secret_headers: Vec<McpHeaderName>,
}

impl TeamMcpRemote {
  pub fn new(url: McpUrl, secret_headers: Vec<McpHeaderName>) -> Result<Self, TeamMcpServerError> {
    Ok(Self {
      url,
      secret_headers: distinct(secret_headers, McpHeaderName::as_str)?,
    })
  }

  pub const fn url(&self) -> &McpUrl {
    &self.url
  }

  pub const fn secret_headers(&self) -> &Vec<McpHeaderName> {
    &self.secret_headers
  }
}

/// How the server runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamMcpLaunch {
  Program(TeamMcpProgram),
  Remote(TeamMcpRemote),
}

/// An MCP server of a team project, as a versioned resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "wire::TeamMcpServerWire", into = "wire::TeamMcpServerWire")]
pub struct TeamMcpServer {
  name: McpServerName,
  launch: TeamMcpLaunch,
  exempt_tools: ExemptTools,
}

impl TeamMcpServer {
  pub const fn new(name: McpServerName, launch: TeamMcpLaunch, exempt_tools: ExemptTools) -> Self {
    Self {
      name,
      launch,
      exempt_tools,
    }
  }

  pub const fn name(&self) -> &McpServerName {
    &self.name
  }

  pub const fn launch(&self) -> &TeamMcpLaunch {
    &self.launch
  }

  /// The tools that run without asking.
  pub const fn exempt_tools(&self) -> &ExemptTools {
    &self.exempt_tools
  }
}

impl ResourceBody for TeamMcpServer {
  const KIND: ResourceKind = ResourceKind::McpServer;
  const SCHEMA: u32 = 1;
}

/// A computer switches a server on itself, so a version widens only what an already switched-on server may do: it
/// runs another program or reaches another address, or asks less.
impl SharedBody for TeamMcpServer {
  fn widenings_over(&self, previous: Option<&Self>) -> Vec<VersionWidening> {
    let Some(previous) = previous else {
      return Vec::new();
    };
    let mut widenings = Vec::new();
    let launch_changed = match (&self.launch, &previous.launch) {
      (TeamMcpLaunch::Program(now), TeamMcpLaunch::Program(before)) => (now.program != before.program
        || now.args != before.args)
        .then_some(VersionWidening::Command(CommandWidening::Program)),
      (TeamMcpLaunch::Remote(now), TeamMcpLaunch::Remote(before)) => {
        (now.url != before.url).then_some(VersionWidening::Address)
      }
      _ => Some(VersionWidening::McpServer),
    };
    widenings.extend(launch_changed);
    if self
      .exempt_tools
      .tools()
      .iter()
      .any(|tool| !previous.exempt_tools.contains(tool.as_str()))
    {
      widenings.push(VersionWidening::Access);
    }
    widenings
  }
}

fn is_rooted(path: &str) -> bool {
  let path = std::path::Path::new(path);
  path.is_absolute() || path.has_root() || path.to_string_lossy().chars().nth(1) == Some(':')
}

/// `items` in order, each name once; names are told apart without regard to case, as environment variables are on
/// Windows and header names always are.
fn distinct<T>(items: Vec<T>, text: fn(&T) -> &str) -> Result<Vec<T>, TeamMcpServerError> {
  let mut seen = Vec::with_capacity(items.len());
  for item in &items {
    let name = text(item).to_ascii_lowercase();
    if seen.contains(&name) {
      return Err(TeamMcpServerError::Duplicate(text(item).to_owned()));
    }
    seen.push(name);
  }
  Ok(items)
}
