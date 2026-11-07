//! The team's MCP server: what the team server holds of one. A program is named, never located (each computer sets
//! the path of its own), and variables and secret headers name the team secrets whose values each computer fills in:
//! a variable takes the secret of its own name, a secret header names its secret and an optional fixed prefix. The
//! wire shape refuses any other field, so a path, a value or a local id cannot be sent by mistake.
//! `docs/design/team-configuration.md`, "Who owns what".

mod header;
mod wire;

pub use header::{HeaderPrefix, HeaderPrefixError, TeamMcpSecretHeader};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{CommandArg, CommandError, CommandProgram, ProgramLocation, ResourceBody, ResourceKind, VariableName};
use crate::{
  connection::mcp::{
    ExemptTools, McpAuthorizationError, McpServerName, McpServerNameError, McpUrl, TransportError, WorkingDirectory,
  },
  permission::widening::CommandWidening,
  secret::SecretName,
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
  #[error(transparent)]
  Prefix(#[from] HeaderPrefixError),
  /// A variable is filled from the team secret of its own name, so it is named as a secret is.
  #[error("the variable `{0}` is filled from the team secret of the same name, so it must be a valid secret name")]
  VariableNotSecret(String),
  /// A body held before a header named its secret: which secret it takes is not guessed.
  #[error("the secret header `{0}` names no secret; write it as {{\"header\": \"{0}\", \"secret\": NAME}}")]
  HeaderWithoutSecret(String),
  /// The path of a program is set on each computer, so the team names it only: `npx`, not `C:\tools\npx`.
  #[error("the program `{0}` must be a name looked up on the PATH, not a path; each computer sets the path itself")]
  ProgramPath(String),
  /// A folder is relative to the worktree, which is the same on every computer.
  #[error("the working directory `{0}` must be relative to the worktree")]
  AbsoluteFolder(String),
  #[error("`{0}` is listed twice")]
  Duplicate(String),
  /// A key in the query of a URL would go to the team server and to every member. The error holds no URL text for the
  /// same reason: it may carry that key.
  #[error("the URL of a team's MCP server must not have a query or a fragment; a secret goes in a secret header")]
  UrlQuery,
}

/// A program the agent's CLI starts: its name, its arguments, the folder it starts in and the variables it needs, each
/// given the value of the team secret of the same name.
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
    if let Some(variable) = variables
      .iter()
      .find(|variable| SecretName::new(variable.as_str()).is_err())
    {
      return Err(TeamMcpServerError::VariableNotSecret(variable.as_str().to_owned()));
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

  /// The team secret each variable takes its value from: the one of its own name.
  pub fn variable_secret(variable: &VariableName) -> SecretName {
    SecretName::new(variable.as_str()).expect("a variable of a program is checked to be a secret name when built")
  }

  /// The secrets the variables take, in order.
  pub fn secrets(&self) -> Vec<SecretName> {
    self.variables.iter().map(Self::variable_secret).collect()
  }
}

/// A server already running somewhere, with the headers a computer fills in with the values of team secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamMcpRemote {
  url: McpUrl,
  secret_headers: Vec<TeamMcpSecretHeader>,
}

impl TeamMcpRemote {
  pub fn new(url: McpUrl, secret_headers: Vec<TeamMcpSecretHeader>) -> Result<Self, TeamMcpServerError> {
    if url.as_str().contains(['?', '#']) {
      return Err(TeamMcpServerError::UrlQuery);
    }
    Ok(Self {
      url,
      secret_headers: distinct(secret_headers, |header| header.header().as_str())?,
    })
  }

  pub const fn url(&self) -> &McpUrl {
    &self.url
  }

  pub const fn secret_headers(&self) -> &Vec<TeamMcpSecretHeader> {
    &self.secret_headers
  }

  /// The secrets the headers take, in order, each once.
  pub fn secrets(&self) -> Vec<SecretName> {
    let mut secrets: Vec<SecretName> = Vec::with_capacity(self.secret_headers.len());
    for header in &self.secret_headers {
      if !secrets.contains(header.secret()) {
        secrets.push(header.secret().clone());
      }
    }
    secrets
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

  /// The team secrets this server is given the values of, in order, each once: what a computer needs a value for
  /// before the server can run.
  pub fn secrets(&self) -> Vec<SecretName> {
    match &self.launch {
      TeamMcpLaunch::Program(program) => program.secrets(),
      TeamMcpLaunch::Remote(remote) => remote.secrets(),
    }
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
/// runs another program (or in another folder) or reaches another address, is given a team secret the previous version
/// was not, sends a header another secret's value (or with another prefix), or asks less. Dropping any of these
/// narrows.
impl SharedBody for TeamMcpServer {
  fn widenings_over(&self, previous: Option<&Self>) -> Vec<VersionWidening> {
    let Some(previous) = previous else {
      return Vec::new();
    };
    let mut widenings = Vec::new();
    let launch_changed = match (&self.launch, &previous.launch) {
      (TeamMcpLaunch::Program(now), TeamMcpLaunch::Program(before)) => {
        (now.program != before.program || now.args != before.args || now.working_directory != before.working_directory)
          .then_some(VersionWidening::Command(CommandWidening::Program))
      }
      (TeamMcpLaunch::Remote(now), TeamMcpLaunch::Remote(before)) => {
        (now.url != before.url).then_some(VersionWidening::Address)
      }
      _ => Some(VersionWidening::McpServer),
    };
    widenings.extend(launch_changed);
    if self.launch.secrets_beyond(&previous.launch) {
      widenings.push(VersionWidening::Secret);
    }
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

impl TeamMcpLaunch {
  fn secrets(&self) -> Vec<SecretName> {
    match self {
      Self::Program(program) => program.secrets(),
      Self::Remote(remote) => remote.secrets(),
    }
  }

  /// Whether this launch is given a team secret `before` was not, or sends a header of `before` another secret's value
  /// or with another prefix. A launch of the other kind is given all of its own.
  fn secrets_beyond(&self, before: &Self) -> bool {
    let was = before.secrets();
    if self.secrets().iter().any(|secret| !was.contains(secret)) {
      return true;
    }
    let (Self::Remote(now), Self::Remote(before)) = (self, before) else {
      return false;
    };
    now.secret_headers.iter().any(|header| {
      before.secret_headers.iter().any(|old| {
        old.header().as_str().eq_ignore_ascii_case(header.header().as_str())
          && (old.secret() != header.secret() || old.prefix() != header.prefix())
      })
    })
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
