//! A team MCP server as an agent's CLI is given it: launched from the version the run's bundle pins, the program path
//! this computer set for it when the run started (kept in the run's bindings) and the values of the team secrets it
//! takes, read by name in this computer's keychain when the agent starts. A variable takes the value of the secret of
//! its own name; a secret header is sent its prefix followed by its secret's value. A value goes only to the CLI, as a
//! project secret's does.

use std::{collections::HashMap, path::Path, sync::Arc};

use orchester_core::{
  connection::{
    invocation::{HttpAuthorization, LaunchValue, McpLaunch},
    mcp::McpServerProblem,
  },
  resource::{CommandEnvironment, CommandProgram, TeamMcpLaunch, TeamMcpProgram, TeamMcpRemote, TeamMcpServer},
  secret::{SecretName, SecretValue},
};

use super::{no_current_directory, started, unicode};
use crate::computer::ComputerResources;

/// Team secret values read for one invocation on the run's computer, each once.
pub(super) struct TeamSecrets<'a> {
  computer: &'a dyn ComputerResources,
  read: HashMap<SecretName, Result<Arc<SecretValue>, McpServerProblem>>,
}

impl<'a> TeamSecrets<'a> {
  pub(super) fn new(computer: &'a dyn ComputerResources) -> Self {
    Self {
      computer,
      read: HashMap::new(),
    }
  }

  fn read(&mut self, name: &SecretName) -> Result<Arc<SecretValue>, McpServerProblem> {
    if let Some(read) = self.read.get(name) {
      return read.clone();
    }
    let read = self.fetch(name);
    self.read.insert(name.clone(), read.clone());
    read
  }

  fn fetch(&self, name: &SecretName) -> Result<Arc<SecretValue>, McpServerProblem> {
    match self.computer.team_secret_value(name) {
      Ok(Some(value)) => Ok(Arc::new(value)),
      Ok(None) => Err(McpServerProblem::SecretMissing {
        secret: name.to_string(),
      }),
      Err(error) => {
        tracing::warn!(secret = %name, %error, "the keychain did not give a team secret an MCP server uses");
        Err(McpServerProblem::SecretUnreadable {
          secret: name.to_string(),
        })
      }
    }
  }
}

/// `server` at `program` (the path this computer set for it; `None` for a remote server), for an agent working in
/// `worktree` with the project's `environment`.
pub(super) fn launch(
  server: &TeamMcpServer,
  program: Option<&Path>,
  (worktree, repository): (&Path, &Path),
  (environment, secrets): (&CommandEnvironment, &mut TeamSecrets<'_>),
) -> Result<McpLaunch, McpServerProblem> {
  let launch = match server.launch() {
    TeamMcpLaunch::Program(run) => {
      let path = program.ok_or_else(|| McpServerProblem::ProgramNotFound {
        program: run.program().as_str().to_owned(),
      })?;
      started_program(server, run, path, (worktree, repository), (environment, secrets))?
    }
    TeamMcpLaunch::Remote(remote) => remote_launch(server, remote, secrets)?,
  };
  Ok(launch.with_exempt_tools(server.exempt_tools().clone()))
}

fn started_program(
  server: &TeamMcpServer,
  run: &TeamMcpProgram,
  path: &Path,
  (worktree, repository): (&Path, &Path),
  (environment, secrets): (&CommandEnvironment, &mut TeamSecrets<'_>),
) -> Result<McpLaunch, McpServerProblem> {
  let text = unicode(path.to_path_buf())?;
  let program = CommandProgram::new(&text).map_err(|_| McpServerProblem::ProgramNotFound { program: text })?;
  let started = started(&program, run.args(), (worktree, repository), environment)?;
  let working_directory = run
    .working_directory()
    .map(|directory| unicode(directory.resolve(worktree)))
    .transpose()?;
  let mut variables = no_current_directory(run.variables());
  for variable in run.variables() {
    let value = secrets.read(&TeamMcpProgram::variable_secret(variable))?;
    variables.push((variable.clone(), LaunchValue::Secret(value)));
  }
  Ok(McpLaunch::stdio(
    server.name().clone(),
    started,
    working_directory,
    variables,
  ))
}

/// The remote server, sent its one secret header: the prefix and the secret's value. The CLI is given one header per
/// HTTP server, so a server that takes more is left out, and the invocation's activity says so.
fn remote_launch(
  server: &TeamMcpServer,
  remote: &TeamMcpRemote,
  secrets: &mut TeamSecrets<'_>,
) -> Result<McpLaunch, McpServerProblem> {
  let authorization = match remote.secret_headers().as_slice() {
    [] => HttpAuthorization::None,
    [header] => {
      let secret = secrets.read(header.secret())?;
      match header.prefix() {
        None => HttpAuthorization::Header {
          name: header.header().clone(),
          value: secret,
          parts: Vec::new(),
        },
        Some(prefix) => {
          // The prefix is plain text; the whole value is a secret from the moment it is made.
          let whole = SecretValue::new(format!("{}{}", prefix.as_str(), secret.expose())).map_err(|_| {
            McpServerProblem::SecretUnreadable {
              secret: header.secret().to_string(),
            }
          })?;
          HttpAuthorization::Header {
            name: header.header().clone(),
            value: Arc::new(whole),
            parts: vec![secret],
          }
        }
      }
    }
    _ => return Err(McpServerProblem::MoreThanOneSecretHeader),
  };
  Ok(McpLaunch::http(
    server.name().clone(),
    remote.url().clone(),
    authorization,
  ))
}
