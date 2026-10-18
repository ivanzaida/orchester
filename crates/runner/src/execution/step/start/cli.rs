//! The agent CLI's command: Codex App Server with what the runner tells it once it runs, or Claude
//! Code with its prompt on standard input. The repository's skills Codex disables while the
//! project has its agent files off are found here too.

use std::{
  collections::BTreeSet,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  agent_files::skills::RepositorySkills,
  command::Command,
  connection::{
    AgentConnection, AgentProvider, ConnectionAccount,
    invocation::{
      CODEX_PROVIDER_ID, ClaudeInvocation, ClaudeMcp, ClaudeSession, CodexAgentFiles, CodexInvocation, CodexMcp,
      InvocationError, OnProvider, Platform, on_account,
    },
    stream::RunnerProblem,
  },
  orchester_tools::{ProjectTool, SERVER},
  permission::{CommandPermission, OutsideCommands, capability::EnforcedPermissions},
  resource::{CommandEnvironment, ModelId},
  store::SettingsStore,
};

use super::{
  super::{
    super::run::RunTask,
    app_server::AppServerSetup,
    mcp::{ClaudeServers, McpSetup},
    orchester_tools::{CodexTools, ToolHost},
    relay::Listening,
  },
  NATIVE_NOT_AVAILABLE, problem,
};
use crate::private_file::PrivateFile;

/// The CLI's command, and what must live as long as the CLI: Claude Code's MCP configuration file, or
/// what the runner tells Codex App Server and the listener of its relay of Orchester's tools.
pub(super) struct CliCommand {
  pub(super) command: Command,
  pub(super) mcp_file: Option<PrivateFile>,
  pub(super) app_server: Option<AppServerSetup>,
  pub(super) tool_relay: Option<Listening>,
}

/// What decides the CLI's arguments besides its prompt and MCP servers.
pub(super) struct Cli<'a> {
  /// The CLI's program.
  pub(super) executable: &'a Path,
  /// Whose sign-in it runs on: the account's folder variable is set over the environment's and the project's.
  pub(super) account: &'a ConnectionAccount,
  /// The API provider it runs on instead, with its key: its variables and overrides are set last.
  pub(super) provider: Option<&'a OnProvider<'a>>,
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
  /// The checks the step is offered, which Claude Code's allow rules follow.
  pub(super) checks: &'a [ProjectTool],
}

/// The program of the agent CLI `connection` runs, or why no agent starts on it: this version runs
/// no native agent, and none starts without an account, on an account whose folder is gone, or on one this
/// platform does not support yet (a connection changed after the run's preflight).
pub(super) fn program(connection: &AgentConnection) -> Result<&Path, RunnerProblem> {
  if let Some(reason) = connection
    .account()
    .start_problem(connection.provider(), Platform::current(), Path::is_dir)
  {
    return Err(problem(reason.to_string()));
  }
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
      (false, AgentProvider::CodexCli) => {
        let skills = RepositorySkills::find(worktree).map_err(|error| problem(error.refusal_while_off()))?;
        Some(skills.codex_paths())
      }
      _ => None,
    })
  }

  /// The CLI's command with `prompt`, the MCP servers of `setup` and the Orchester tools of `tools`; Claude
  /// Code's MCP configuration file, or what the runner tells Codex App Server once it runs.
  pub(super) fn cli_command(
    &self,
    cli: &Cli<'_>,
    setup: McpSetup,
    prompt: String,
    tools: Arc<ToolHost>,
  ) -> Result<CliCommand, RunnerProblem> {
    tools
      .processes
      .protect_executable(cli.executable)
      .map_err(|error| problem(format!("cannot protect agent CLI: {error}")))?;
    match setup {
      McpSetup::Codex(mcp) => {
        let CodexTools { server, listening } = self.codex_tools(tools)?;
        let mcp = mcp.with_orchester(server);
        let mcp = if cli.enforced.permissions().interactive_tool_approvals() {
          mcp
        } else {
          mcp.without_exemptions()
        };
        let (command, app) = self.app_server_command(cli, &mcp, prompt)?;
        Ok(CliCommand {
          command,
          mcp_file: None,
          app_server: Some(app),
          tool_relay: Some(listening),
        })
      }
      McpSetup::Claude(servers) => {
        let (command, mcp_file) = self.claude_command(cli, servers, &prompt)?;
        Ok(CliCommand {
          command,
          mcp_file,
          app_server: None,
          tool_relay: None,
        })
      }
    }
  }

  /// Codex App Server with the MCP servers `mcp`, Orchester's among them, and what the runner tells it
  /// once it runs: the prompt, the thread's settings and the approvals it may ask a person for.
  fn app_server_command(
    &self,
    cli: &Cli<'_>,
    mcp: &CodexMcp,
    prompt: String,
  ) -> Result<(Command, AppServerSetup), RunnerProblem> {
    let mut expected_servers = mcp.servers().iter().map(ToString::to_string).collect::<BTreeSet<_>>();
    expected_servers.insert(SERVER.to_owned());
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
      .and_then(|command| on_account(command, cli.account, AgentProvider::CodexCli))
      .map(|command| on_provider(command, cli.provider, AgentProvider::CodexCli))
      .map_err(|error| problem(error.to_string()))?;
    let mut secrets = mcp.secrets().to_vec();
    secrets.extend(cli.provider.map(|on| on.key_variable(AgentProvider::CodexCli)));
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
        exemptions: mcp.exemptions().clone(),
        output_schema: schema,
        secrets,
        model_provider: cli.provider.map(|_| CODEX_PROVIDER_ID.to_owned()),
      },
    ))
  }

  /// Claude Code's command with `prompt` on its standard input and the MCP servers `servers`, and the
  /// configuration file it reads them from, if any.
  fn claude_command(
    &self,
    cli: &Cli<'_>,
    servers: Option<ClaudeServers>,
    prompt: &str,
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
      checks: cli.checks,
    }
    .command(prompt, std::env::vars_os(), cli.environment)
    .and_then(|command| on_account(command, cli.account, AgentProvider::ClaudeCodeCli))
    .map(|command| on_provider(command, cli.provider, AgentProvider::ClaudeCodeCli))
    .map_err(|error: InvocationError| problem(error.to_string()))?;
    Ok((command, servers.map(|servers| servers.file)))
  }
}

/// `command`, the CLI's of `cli`, on `provider` if it runs on one.
fn on_provider(command: Command, provider: Option<&OnProvider<'_>>, cli: AgentProvider) -> Command {
  match provider {
    Some(on) => on.apply(command, cli),
    None => command,
  }
}
