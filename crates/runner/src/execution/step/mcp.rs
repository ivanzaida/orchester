//! The MCP servers of an agent invocation: those its profile chooses, as far as they can be given
//! to the agent's CLI (`orchester_core::connection::invocation::mcp`). A server's program is found
//! the way a command tool's is, on the `PATH` the CLI starts with or in the worktree, and started
//! the way `orchester-exec` would start it (npm's Windows launchers through Node.js, other command
//! scripts through `cmd.exe`). Secret values are read from the keychain now, once per secret, and
//! go only into the CLI's environment (Codex) or its private configuration file (Claude Code).
//! A server that cannot be given is left out, and the invocation's activity says why; the agent
//! works without it.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  connection::{
    AgentProvider,
    invocation::{ClaudeMcpConfig, CodexMcp, LaunchValue, McpLaunch, Platform},
    mcp::{McpEnvValue, McpServer, McpServerName, McpServerProblem, McpTransport, StdioServer},
    stream::{AgentEvent, RunnerProblem},
  },
  id::{InvocationId, SecretId},
  resource::{AgentProfile, CommandEnvironment},
  secret::{SecretValue, SecretVault},
  store::{McpServerStore, SecretStore},
};
use orchester_exec::launch::{self, Via};
use orchester_store::ProjectDb;

use super::super::run::RunTask;
use crate::private_file::PrivateFile;

/// A server left out, by name (by id when the project no longer has it), and why.
type LeftOut = (String, McpServerProblem);

/// How the invocation's CLI is given its servers.
pub(in super::super) enum McpSetup {
  Codex(CodexMcp),
  /// The configuration file, and the servers in it; `None` when no server is left.
  Claude(Option<(PrivateFile, Vec<McpServerName>)>),
}

/// The servers of one invocation, and an event for each one left out.
pub(in super::super) struct McpServers {
  pub(in super::super) setup: McpSetup,
  pub(in super::super) left_out: Vec<AgentEvent>,
}

impl RunTask {
  /// The MCP servers `profile` chooses, for invocation `invocation` of a `provider` agent working in
  /// `worktree` with the project's `environment`.
  pub(super) fn mcp_servers(
    &self,
    profile: &AgentProfile,
    provider: AgentProvider,
    (invocation, worktree, environment): (InvocationId, &Path, &CommandEnvironment),
  ) -> Result<McpServers, RunnerProblem> {
    let (launches, mut left_out) = self.mcp_launches(profile, worktree, environment)?;
    let setup = match provider {
      AgentProvider::CodexCli => {
        let (mcp, more) = CodexMcp::new(&launches);
        left_out.extend(named(more));
        McpSetup::Codex(mcp)
      }
      AgentProvider::ClaudeCodeCli => {
        let (config, more) = ClaudeMcpConfig::new(&launches);
        left_out.extend(named(more));
        let file = config
          .map(|config| {
            PrivateFile::write(self.project.id(), &format!("{invocation}.json"), config.contents())
              .map(|file| (file, config.servers().to_vec()))
              .map_err(|error| RunnerProblem::CouldNotStart {
                detail: format!("cannot write the MCP server configuration: {error}"),
              })
          })
          .transpose()?;
        McpSetup::Claude(file)
      }
    };
    let left_out = left_out
      .into_iter()
      .map(|(server, problem)| AgentEvent::McpServer {
        server: Some(server),
        problem,
      })
      .collect();
    Ok(McpServers { setup, left_out })
  }

  fn mcp_launches(
    &self,
    profile: &AgentProfile,
    worktree: &Path,
    environment: &CommandEnvironment,
  ) -> Result<(Vec<McpLaunch>, Vec<LeftOut>), RunnerProblem> {
    let mut secrets = Secrets {
      db: &self.db,
      vault: self.vault.as_ref(),
      project: self.project.id(),
      read: HashMap::new(),
    };
    let (mut launches, mut left_out) = (Vec::new(), Vec::new());
    for id in profile.mcp_servers().ids() {
      let server = self.db.mcp_server(*id).map_err(|error| RunnerProblem::CouldNotStart {
        detail: error.to_string(),
      })?;
      let Some(server) = server else {
        left_out.push((id.to_string(), McpServerProblem::Missing));
        continue;
      };
      if !server.enabled() {
        left_out.push((server.name().to_string(), McpServerProblem::Disabled));
        continue;
      }
      match launched(&server, worktree, environment, &mut secrets) {
        Ok(launch) => launches.push(launch),
        Err(problem) => left_out.push((server.name().to_string(), problem)),
      }
    }
    Ok((launches, left_out))
  }
}

fn named(left_out: Vec<(McpServerName, McpServerProblem)>) -> impl Iterator<Item = LeftOut> {
  left_out.into_iter().map(|(name, problem)| (name.to_string(), problem))
}

/// `server` as the CLI is given it, with its program found and its secrets read.
fn launched(
  server: &McpServer,
  worktree: &Path,
  environment: &CommandEnvironment,
  secrets: &mut Secrets<'_>,
) -> Result<McpLaunch, McpServerProblem> {
  let stdio = match server.transport() {
    McpTransport::Http(http) => {
      let bearer = http.bearer().map(|secret| secrets.read(secret)).transpose()?;
      return Ok(McpLaunch::http(server.name().clone(), http.url().clone(), bearer));
    }
    McpTransport::Stdio(stdio) => stdio,
  };
  let program = started(stdio, worktree, environment)?;
  let working_directory = stdio
    .working_directory()
    .map(|directory| unicode(directory.resolve(worktree)))
    .transpose()?;
  let mut variables = Vec::new();
  for (name, value) in server.environment().iter() {
    let value = match value {
      McpEnvValue::Text(text) => LaunchValue::Text(text.clone()),
      McpEnvValue::Secret(secret) => LaunchValue::Secret(secrets.read(*secret)?),
    };
    variables.push((name.clone(), value));
  }
  Ok(McpLaunch::stdio(
    server.name().clone(),
    program,
    working_directory,
    variables,
  ))
}

/// The program that starts and its arguments: the file the server's program is on the `PATH`
/// the CLI starts with (the project's, else Orchester's search path, which on macOS adds the usual install
/// folders) or in the worktree, started as
/// `orchester-exec` would start it.
fn started(
  stdio: &StdioServer,
  worktree: &Path,
  environment: &CommandEnvironment,
) -> Result<(String, Vec<String>), McpServerProblem> {
  let path = environment
    .get("PATH")
    .map(Into::into)
    .or_else(orchester_exec::search_path::search_path);
  let found = stdio
    .program()
    .candidates(worktree, path.as_deref(), Platform::current())
    .into_iter()
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| McpServerProblem::ProgramNotFound {
      program: stdio.program().as_str().to_owned(),
    })?;
  let args: Vec<String> = stdio.args().iter().map(|arg| arg.as_str().to_owned()).collect();
  let launch = launch::resolve(&found);
  if let Err(orchester_core::command::CommandError::UnsafeScriptArgument { position }) = launch.check_args(&args) {
    return Err(McpServerProblem::UnsafeScriptArgument { position });
  }
  let leading = launch.leading_args().to_vec();
  match launch.via() {
    Via::Direct | Via::NodeScript { .. } => Ok((unicode(launch.program().to_path_buf())?, [leading, args].concat())),
    // The CLI starts programs, not scripts: `cmd.exe` runs the script, with arguments that pass
    // through it unchanged (checked above).
    Via::CommandScript => {
      let shell = std::env::var_os("COMSPEC").ok_or_else(|| McpServerProblem::ProgramNotFound {
        program: "cmd.exe".to_owned(),
      })?;
      let script = unicode(found)?;
      let head = ["/d".to_owned(), "/c".to_owned(), script];
      Ok((unicode(PathBuf::from(shell))?, [head.to_vec(), args].concat()))
    }
  }
}

fn unicode(path: PathBuf) -> Result<String, McpServerProblem> {
  path
    .into_os_string()
    .into_string()
    .map_err(|_| McpServerProblem::NotUnicode)
}

/// Secret values read for one invocation, each once.
struct Secrets<'a> {
  db: &'a ProjectDb,
  vault: &'a dyn SecretVault,
  project: orchester_core::id::ProjectId,
  read: HashMap<SecretId, Result<Arc<SecretValue>, McpServerProblem>>,
}

impl Secrets<'_> {
  fn read(&mut self, id: SecretId) -> Result<Arc<SecretValue>, McpServerProblem> {
    if let Some(read) = self.read.get(&id) {
      return read.clone();
    }
    let read = self.fetch(id);
    self.read.insert(id, read.clone());
    read
  }

  fn fetch(&self, id: SecretId) -> Result<Arc<SecretValue>, McpServerProblem> {
    let secret = match self.db.secret(id) {
      Ok(Some(secret)) => secret,
      Ok(None) => return Err(McpServerProblem::SecretMissing { secret: id.to_string() }),
      Err(error) => {
        tracing::warn!(%id, %error, "cannot read a secret an MCP server uses");
        return Err(McpServerProblem::SecretUnreadable { secret: id.to_string() });
      }
    };
    let name = secret.name().to_string();
    if secret.is_archived() {
      return Err(McpServerProblem::SecretArchived { secret: name });
    }
    self
      .vault
      .read(&secret.vault_key(self.project))
      .map(Arc::new)
      .map_err(|error| {
        tracing::warn!(secret = %name, %error, "the keychain did not give a secret an MCP server uses");
        McpServerProblem::SecretUnreadable { secret: name }
      })
  }
}
