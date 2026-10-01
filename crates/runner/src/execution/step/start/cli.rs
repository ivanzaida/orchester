//! The agent CLI's command: Codex App Server with what the runner tells it once it runs, or Claude
//! Code with its prompt on standard input. The repository's skills Codex disables while the
//! project has its agent files off are found here too.

use std::{
  collections::BTreeSet,
  path::{Path, PathBuf},
};

use orchester_core::{
  command::Command,
  connection::{
    AgentConnection, AgentProvider,
    invocation::{
      ClaudeInvocation, ClaudeMcp, ClaudeSession, CodexAgentFiles, CodexInvocation, CodexMcp, InvocationError, Platform,
    },
    mcp::InvocationExemptions,
    stream::RunnerProblem,
  },
  permission::{CommandPermission, OutsideCommands, capability::EnforcedPermissions},
  resource::{CommandEnvironment, ModelId},
  store::SettingsStore,
};

use super::{
  super::{super::run::RunTask, app_server::AppServerSetup, mcp::ClaudeServers, skills::repository_skills},
  NATIVE_NOT_AVAILABLE, problem,
};
use crate::private_file::PrivateFile;

/// What decides the CLI's arguments besides its prompt and MCP servers.
pub(super) struct Cli<'a> {
  /// The CLI's program.
  pub(super) executable: &'a Path,
  pub(super) model: &'a ModelId,
  pub(super) effort: Option<&'a str>,
  pub(super) enforced: &'a EnforcedPermissions,
  pub(super) writable_roots: &'a [PathBuf],
  pub(super) worktree: &'a Path,
  pub(super) resume: Option<&'a str>,
  pub(super) environment: &'a CommandEnvironment,
  /// The folder of the skills the agent gets, if any.
  pub(super) skills: Option<&'a Path>,
  /// The repository's skills Codex disables, when the project has its agent files off; `None` while
  /// they are on.
  pub(super) repository_skills: Option<&'a [PathBuf]>,
  /// The code index folder the agent gets, if any.
  pub(super) code_index: Option<&'a Path>,
  /// Claude Code's permission prompt configuration, when the step asks a person.
  pub(super) approvals: Option<&'a Path>,
}

/// The program of the agent CLI `connection` runs, or why no agent starts on it: this version runs
/// no native agent.
pub(super) fn program(connection: &AgentConnection) -> Result<&Path, RunnerProblem> {
  match connection.executable() {
    Some(executable) if connection.provider().runs_agents() => Ok(executable),
    _ => Err(problem(NATIVE_NOT_AVAILABLE.to_owned())),
  }
}

impl RunTask {
  /// The repository's skills Codex disables while the project has its agent files off; `None`
  /// while they are on, or for Claude Code.
  pub(super) fn repository_skills_left_out(
    &self,
    provider: AgentProvider,
    worktree: &Path,
  ) -> Result<Option<Vec<PathBuf>>, RunnerProblem> {
    let agent_files = self
      .db
      .repository_agent_files()
      .map_err(|error| problem(error.to_string()))?;
    Ok(match (agent_files.is_on(), provider) {
      (false, AgentProvider::CodexCli) => Some(repository_skills(worktree)?),
      _ => None,
    })
  }

  /// Codex App Server with the MCP servers `mcp`, and what the runner tells it once it runs: the
  /// prompt, the thread's settings and the approvals it may ask a person for.
  pub(super) fn app_server_command(
    &self,
    cli: &Cli<'_>,
    mcp: &CodexMcp,
    prompt: String,
  ) -> Result<(Command, AppServerSetup), RunnerProblem> {
    let expected_servers = mcp.servers().iter().map(ToString::to_string).collect::<BTreeSet<_>>();
    let invocation = CodexInvocation {
      executable: cli.executable,
      repository: &self.root,
      access: cli.enforced.permissions().repository(),
      network: cli.enforced.permissions().network(),
      writable_roots: cli.writable_roots,
      worktree: cli.worktree,
      platform: Platform::current(),
      mcp: Some(mcp),
      agent_files: cli
        .repository_skills
        .map_or(CodexAgentFiles::Loaded, |skills| CodexAgentFiles::LeftOut { skills }),
    };
    let command = invocation
      .command(std::env::vars_os(), cli.environment)
      .map_err(|error| problem(error.to_string()))?;
    let schema = (cli.resume.is_none()).then(|| {
      serde_json::from_str(orchester_core::run::STEP_RESULT_SCHEMA)
        .expect("the static step result schema is valid JSON")
    });
    Ok((
      command,
      AppServerSetup {
        prompt,
        model: cli.model.as_str().to_owned(),
        effort: cli.effort.map(str::to_owned),
        worktree: cli.worktree.to_path_buf(),
        access: cli.enforced.permissions().repository(),
        command_approvals: *cli.enforced.permissions().commands() == CommandPermission::AskBefore,
        sandbox_approvals: cli.enforced.outside_commands() == OutsideCommands::SandboxApproval,
        tool_approvals: cli.enforced.permissions().interactive_tool_approvals(),
        network: cli.enforced.permissions().network(),
        writable_roots: cli.writable_roots.to_vec(),
        resume: cli.resume.map(str::to_owned),
        expected_servers,
        exemptions: if cli.enforced.permissions().interactive_tool_approvals() {
          mcp.exemptions().clone()
        } else {
          InvocationExemptions::default()
        },
        output_schema: schema,
        secrets: mcp.secrets().to_vec(),
      },
    ))
  }

  /// Claude Code's command with `prompt` on its standard input and the MCP servers `servers`, and the
  /// configuration file it reads them from, if any.
  pub(super) fn claude_command(
    &self,
    cli: &Cli<'_>,
    servers: Option<ClaudeServers>,
    prompt: String,
  ) -> Result<(Command, Option<PrivateFile>), RunnerProblem> {
    let command = ClaudeInvocation {
      executable: cli.executable,
      model: cli.model.as_str(),
      effort: cli.effort,
      permissions: cli.enforced,
      writable_roots: cli.writable_roots,
      worktree: cli.worktree,
      session: ClaudeSession::continuing(cli.resume),
      mcp: servers.as_ref().map(|servers| ClaudeMcp {
        config: servers.file.path(),
        servers: &servers.servers,
        exempt: &servers.exemptions,
      }),
      skills: cli.skills,
      code_index: cli.code_index,
      approvals: cli.approvals,
    }
    .command(prompt, std::env::vars_os(), cli.environment)
    .map_err(|error: InvocationError| problem(error.to_string()))?;
    Ok((command, servers.map(|servers| servers.file)))
  }
}
