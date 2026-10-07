//! How an agent step's invocation starts, for the provider of its profile's connection (Codex, Claude
//! Code or a native agent, [`native`]): the connection and settings, the worktree it works in, the variables the
//! project adds to its environment (read at each start), the MCP servers the profile chooses (`mcp`), and
//! the prompt assembled for its injection event ([`prompt`]). The CLI's command is built in
//! [`cli`]; what every provider shares (the step, its profile and connection, the permissions and
//! writable folders) in [`shared`]. The skills that reach the agent are placed and listed in its
//! prompt, and the project's repository agent files setting is read at each start
//! ([`super::skills`]). What the agent gets of the code index is decided before ([`AgentIndex`]):
//! its folder and the internal skill in the prompt of new and resumed sessions (Claude Code also
//! gets the folder with `--add-dir`), and ripgrep's folder first on the CLI's `PATH`.

mod cli;
mod native;
mod prompt;
mod retry;
mod shared;

use std::path::{Path, PathBuf};

use orchester_core::{
  code_index::CodeIndexUse,
  command::{Command, SecretVariable},
  connection::{
    AgentConnection, REASONING_EFFORT,
    mcp::InvocationExemptions,
    stream::{AgentEvent, ClaudeStream, RunnerProblem},
  },
  id::{InvocationId, StepExecutionId},
  memory::{InjectedMemory, RecalledMemory},
  permission::LaunchPermissions,
  resource::{AgentProfile, ModelId, Step},
  run::{
    AssembledPrompt, CodeIndexPrompt, InvocationEngine, InvocationPrompt, PinnedVersion, ProfileInfo, WorkingTools,
  },
};

use self::cli::Cli;
use super::{
  super::{
    codebase_index::{AgentIndex, IndexTold, with_tools_on_path},
    run::RunTask,
  },
  Continuation,
  app_server::{AppServerSession, AppServerSetup},
  claude_approval::ClaudeApprovals,
  mcp::{McpServers, McpSetup},
  native::{NativeHandle, NativeSession},
  orchester_tools::{ToolFolders, ToolHost},
  relay::Listening,
  skills::PlacedSkills,
  worker::{Driver, Transport},
};
use crate::{
  private_file::PrivateFile,
  supervisor::{Supervised, SupervisorError},
};

/// Why an agent on a native connection does not start: preflight refuses such steps, and an agent
/// never starts on a connection this version cannot run.
pub(super) const NATIVE_NOT_AVAILABLE: &str = "native agents are not available in this version";

pub(super) fn problem(detail: String) -> RunnerProblem {
  RunnerProblem::CouldNotStart { detail }
}

/// How an agent invocation starts.
pub(super) struct AgentStart {
  pub(super) launch: Launch,
  pub(super) prompt: InvocationPrompt,
  /// An event for each MCP server the agent works without, and for the code index it lacks.
  pub(super) left_out: Vec<AgentEvent>,
  /// What the agent gets of the code index.
  pub(super) code_index: CodeIndexUse,
  /// Secret values kept out of the invocation's activity: Claude Code's MCP servers', a native
  /// agent's API key.
  pub(super) secrets: Vec<SecretVariable>,
  pub(super) agent: String,
  /// The task memory its prompt holds, to record with the invocation.
  pub(super) memory: Vec<InjectedMemory>,
  pub(super) permissions: LaunchPermissions,
  /// What a native agent reaches its model through; `None` for a CLI.
  pub(super) engine: Option<InvocationEngine>,
}

/// What starts: an agent CLI's process, or a native agent's loop.
pub(super) enum Launch {
  Cli(Box<CliLaunch>),
  Native(Box<NativeSession>),
}

/// A started agent and what its worker follows it with.
pub(super) struct Driven {
  pub(super) driver: Driver,
  /// Claude Code's MCP configuration file, removed when this is dropped: once the CLI has ended.
  pub(super) mcp_file: Option<PrivateFile>,
  pub(super) claude_approvals: Option<ClaudeApprovals>,
  /// Codex's relay of Orchester's tools, which asks about their calls.
  pub(super) tool_relay: Option<Listening>,
  /// Whether the agent can ask a person to decide about a tool call.
  pub(super) interactive: bool,
  /// Whether people's messages sent now reach the agent while it works; a native agent reads them
  /// once its turn is over.
  pub(super) steered: bool,
}

impl Launch {
  /// Starts the CLI's process, or the native agent's task.
  pub(super) fn start(self) -> Result<Driven, SupervisorError> {
    match self {
      Self::Cli(cli) => {
        let CliLaunch {
          command,
          mcp_file,
          app_server,
          claude_approvals,
          tool_relay,
          claude_exemptions,
        } = *cli;
        let process = Supervised::start(&command)?;
        let interactive = app_server.is_some() || claude_approvals.is_some();
        let transport = match app_server {
          Some(setup) => Transport::AppServer(Box::new(AppServerSession::new(setup))),
          None => Transport::Claude(Box::new(ClaudeStream::with_exemptions(claude_exemptions))),
        };
        Ok(Driven {
          driver: Driver::Process { transport, process },
          mcp_file,
          claude_approvals,
          tool_relay,
          interactive,
          steered: true,
        })
      }
      Self::Native(session) => Ok(Driven {
        driver: Driver::Native(NativeHandle::spawn(*session)),
        mcp_file: None,
        claude_approvals: None,
        tool_relay: None,
        interactive: true,
        steered: false,
      }),
    }
  }
}

/// An agent CLI's process and what the runner follows it with.
pub(super) struct CliLaunch {
  pub(super) command: Command,
  /// Claude Code's MCP configuration file, removed when this is dropped: once the CLI has ended.
  pub(super) mcp_file: Option<PrivateFile>,
  pub(super) app_server: Option<AppServerSetup>,
  /// Claude Code's permission prompt, for a step that asks a person.
  pub(super) claude_approvals: Option<ClaudeApprovals>,
  /// Codex's relay of Orchester's tools, which asks about their calls.
  pub(super) tool_relay: Option<Listening>,
  /// The tools of Claude Code's MCP servers that run without asking, when the step asks before MCP actions.
  pub(super) claude_exemptions: InvocationExemptions,
}

/// The agent step an invocation runs: its step, the profile version that runs it and the model, and the
/// connection it runs on.
#[derive(Clone, Copy)]
struct StepAgent<'a> {
  step: &'a Step,
  profile: &'a PinnedVersion,
  model: &'a ModelId,
  body: &'a AgentProfile,
  connection: &'a AgentConnection,
}

impl<'a> StepAgent<'a> {
  const fn info(&self) -> ProfileInfo<'a> {
    ProfileInfo {
      id: self.profile.resource,
      version: self.profile.number,
      body: self.body,
    }
  }
}

impl RunTask {
  /// The internal code index skill, placed among `placed` when the agent gets the index.
  fn place_code_map_for(
    &self,
    index: &AgentIndex,
    execution: StepExecutionId,
    placed: &mut PlacedSkills,
  ) -> Result<Option<std::path::PathBuf>, RunnerProblem> {
    match index.folder {
      Some(_) => self.place_code_map(execution, placed).map(Some),
      None => Ok(None),
    }
  }
}

/// What the prompt of an agent step's invocation is made of besides the run.
#[derive(Clone, Copy)]
struct PromptParts<'a> {
  execution: StepExecutionId,
  step: &'a Step,
  profile: ProfileInfo<'a>,
  continuation: &'a Continuation,
  placed: &'a PlacedSkills,
  code_index: Option<CodeIndexPrompt<'a>>,
  memory: &'a [RecalledMemory],
  working: Option<WorkingTools>,
}

impl RunTask {
  /// How invocation `invocation` of `execution`'s agent starts: its command, the CLI it starts,
  /// what its prompt is made of and its MCP servers, continuing as `continuation` says, with what it
  /// gets of the code index (`index`), or why it cannot start.
  pub(super) fn command(
    &self,
    execution: StepExecutionId,
    continuation: &Continuation,
    (invocation, worktree): (InvocationId, &Path),
    index: AgentIndex,
  ) -> Result<AgentStart, RunnerProblem> {
    let (step, profile, model) = self.agent_step(execution)?;
    let (body, connection) = self.profile_and_connection(profile)?;
    let agent = StepAgent {
      step,
      profile,
      model,
      body: &body,
      connection: &connection,
    };
    if connection.provider().is_native() {
      self.native_start(execution, continuation, (invocation, worktree), index, agent)
    } else {
      self.cli_start(execution, continuation, (invocation, worktree), index, agent)
    }
  }

  /// How an agent CLI's invocation starts; see [`Self::command`].
  fn cli_start(
    &self,
    execution: StepExecutionId,
    continuation: &Continuation,
    (invocation, worktree): (InvocationId, &Path),
    index: AgentIndex,
    agent: StepAgent<'_>,
  ) -> Result<AgentStart, RunnerProblem> {
    let (step, model, body, connection) = (agent.step, agent.model, agent.body, agent.connection);
    let key = &step.key;
    let provider = connection.provider();
    let executable = cli::program(connection)?;
    let info = agent.info();
    let mut placed = self.place_skills(execution, step, info)?;
    let skill = self.place_code_map_for(&index, execution, &mut placed)?;
    let told = IndexTold::new(&index, skill.as_deref(), provider);
    let recalled = self.recall_memory(step, info)?;
    let enforced = self.enforced(key, body, provider)?;
    let parts = PromptParts {
      execution,
      step,
      profile: info,
      continuation,
      placed: &placed,
      code_index: told.prompt(),
      memory: recalled.entries(),
      working: WorkingTools::new(provider, enforced.permissions()),
    };
    let AssembledPrompt { text: prompt, sources } = self.prompt(parts)?;
    let writable = self.writable_folders(&enforced)?;
    let writable_roots: Vec<PathBuf> = writable.iter().map(|folder| folder.path().to_path_buf()).collect();
    let environment = self.environment()?;
    let repository = self.repository_skills_left_out(provider, worktree)?;
    let McpServers { setup, left_out } = self.mcp_servers(body, provider, (invocation, worktree, &environment))?;
    let folders = ToolFolders::new(worktree, &writable, (index.folder.as_deref(), placed.folder.as_deref()));
    let (tools, claude_approvals) =
      self.step_tools((enforced.permissions(), &setup), (invocation, folders), &environment)?;
    let prompt = told_of_tools(prompt, &tools, (&setup, claude_approvals.is_some(), continuation));
    let secrets = setup.claude_secrets();
    let (claude_exemptions, unapplied) = setup.claude_exemptions(enforced.permissions().interactive_tool_approvals());
    let cli = Cli {
      executable,
      model,
      effort: reasoning_effort(body),
      enforced: &enforced,
      writable_roots: &writable_roots,
      worktree,
      resume: continuation.session.as_deref(),
      environment: &environment,
      skills: placed.folder.as_deref(),
      repository_skills: repository.as_deref(),
      code_index: index.folder.as_deref(),
      approvals: claude_approvals.as_ref().map(ClaudeApprovals::config),
    };
    let started = self.cli_command(&cli, setup, prompt, tools)?;
    let mut left_out = left_out;
    left_out.extend(index.problems(provider).into_iter().chain(unapplied));
    Ok(AgentStart {
      launch: Launch::Cli(Box::new(CliLaunch {
        command: match &index.tools {
          Some(tools) => with_tools_on_path(started.command, tools),
          None => started.command,
        },
        mcp_file: started.mcp_file,
        app_server: started.app_server,
        claude_approvals,
        tool_relay: started.tool_relay,
        claude_exemptions,
      })),
      prompt: sources,
      left_out,
      code_index: index.used,
      secrets,
      agent: body.role().as_str().to_owned(),
      memory: recalled.injected(),
      permissions: enforced.launch(),
      engine: None,
    })
  }
}

/// `prompt` with the paragraph that points the agent at its Orchester tools, for a new session that gets them:
/// a Claude Code agent through its permission prompt's relay (`prompted`), every Codex agent.
fn told_of_tools(
  prompt: String,
  tools: &ToolHost,
  (setup, prompted, continuation): (&McpSetup, bool, &Continuation),
) -> String {
  let codex = matches!(setup, McpSetup::Codex(_));
  if continuation.session.is_none() && (prompted || codex) {
    format!("{prompt}\n\n{}", tools.tools().prompt_paragraph(!codex))
  } else {
    prompt
  }
}

/// The reasoning effort `profile` sets, if any.
fn reasoning_effort(profile: &AgentProfile) -> Option<&str> {
  profile
    .settings()
    .iter()
    .find_map(|(name, value)| (name == REASONING_EFFORT).then_some(value))
}
