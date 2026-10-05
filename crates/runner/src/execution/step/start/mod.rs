//! How an agent step's invocation starts, for the provider of its profile's connection (Codex or
//! Claude Code): the connection and settings, the worktree it works in, the variables the project
//! adds to its environment (read at each start), the MCP servers the profile chooses (`mcp`), and
//! the prompt assembled for its injection event ([`prompt`]). The CLI's command is built in
//! [`cli`]; what every provider shares (the step, its profile and connection, the permissions and
//! writable folders) in [`shared`]. The skills that reach the agent are placed and listed in its
//! prompt, and the project's repository agent files setting is read at each start
//! ([`super::skills`]). What the agent gets of the code index is decided before ([`AgentIndex`]):
//! its folder and the internal skill in the prompt of new and resumed sessions (Claude Code also
//! gets the folder with `--add-dir`), and ripgrep's folder first on the CLI's `PATH`.

mod cli;
mod prompt;
mod retry;
mod shared;

use std::path::Path;

use orchester_core::{
  code_index::CodeIndexUse,
  command::{Command, SecretVariable},
  connection::{
    REASONING_EFFORT,
    mcp::InvocationExemptions,
    stream::{AgentEvent, RunnerProblem},
  },
  id::{InvocationId, StepExecutionId},
  memory::{InjectedMemory, RecalledMemory},
  permission::LaunchPermissions,
  resource::{AgentProfile, Step},
  run::{AssembledPrompt, CodeIndexPrompt, InvocationPrompt, ProfileInfo},
};

use self::cli::Cli;
use super::{
  super::{
    codebase_index::{AgentIndex, IndexTold, with_tools_on_path},
    run::RunTask,
  },
  Continuation,
  app_server::AppServerSetup,
  claude_approval::ClaudeApprovals,
  mcp::{McpServers, McpSetup},
  orchester_tools::ToolHost,
  relay::Listening,
  skills::PlacedSkills,
};
use crate::private_file::PrivateFile;

/// Why an agent on a native connection does not start: preflight refuses such steps, and an agent
/// never starts on a connection this version cannot run.
pub(super) const NATIVE_NOT_AVAILABLE: &str = "native agents are not available in this version";

pub(super) fn problem(detail: String) -> RunnerProblem {
  RunnerProblem::CouldNotStart { detail }
}

/// How an agent invocation starts.
pub(super) struct AgentStart {
  pub(super) command: Command,
  pub(super) prompt: InvocationPrompt,
  /// Claude Code's MCP configuration file, removed when this is dropped: once the CLI has ended.
  pub(super) mcp_file: Option<PrivateFile>,
  /// An event for each MCP server the agent works without, and for the code index it lacks.
  pub(super) left_out: Vec<AgentEvent>,
  /// What the agent gets of the code index.
  pub(super) code_index: CodeIndexUse,
  pub(super) app_server: Option<AppServerSetup>,
  /// Claude Code's permission prompt, for a step that asks a person.
  pub(super) claude_approvals: Option<ClaudeApprovals>,
  /// Codex's relay of Orchester's tools, which asks about their calls.
  pub(super) tool_relay: Option<Listening>,
  /// Secret values of Claude Code's MCP servers, kept out of the invocation's activity.
  pub(super) mcp_secrets: Vec<SecretVariable>,
  /// The tools of Claude Code's MCP servers that run without asking, when the step asks before MCP actions.
  pub(super) claude_exemptions: InvocationExemptions,
  pub(super) agent: String,
  /// The task memory its prompt holds, to record with the invocation.
  pub(super) memory: Vec<InjectedMemory>,
  pub(super) permissions: LaunchPermissions,
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
    let key = &step.key;
    let (body, connection) = self.profile_and_connection(profile)?;
    let provider = connection.provider();
    let executable = cli::program(&connection)?;
    let info = ProfileInfo {
      id: profile.resource,
      version: profile.number,
      body: &body,
    };
    let mut placed = self.place_skills(execution, step, info)?;
    let skill = match index.folder {
      Some(_) => Some(self.place_code_map(execution, &mut placed)?),
      None => None,
    };
    let told = IndexTold::new(&index, skill.as_deref(), provider);
    let recalled = self.recall_memory(step, info)?;
    let parts = PromptParts {
      execution,
      step,
      profile: info,
      continuation,
      placed: &placed,
      code_index: told.prompt(),
      memory: recalled.entries(),
    };
    let AssembledPrompt { text: prompt, sources } = self.prompt(parts)?;
    let enforced = self.enforced(key, &body, provider)?;
    let writable_roots = self.writable_roots(&enforced)?;
    let environment = self.environment()?;
    let repository = self.repository_skills_left_out(provider, worktree)?;
    let McpServers { setup, left_out } = self.mcp_servers(&body, provider, (invocation, worktree, &environment))?;
    let (tools, claude_approvals) = self.step_tools(
      (enforced.permissions(), &setup),
      (invocation, worktree, &writable_roots),
      &environment,
    )?;
    // The tools reach a Claude Code agent through its permission prompt's relay, and every Codex agent.
    let served = claude_approvals.is_some() || matches!(setup, McpSetup::Codex(_));
    let prompt = told_of_tools(prompt, &tools, served && continuation.session.is_none());
    let mcp_secrets = setup.claude_secrets();
    let (claude_exemptions, unapplied) = setup.claude_exemptions(enforced.permissions().interactive_tool_approvals());
    let cli = Cli {
      executable,
      model,
      effort: reasoning_effort(&body),
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
      command: match &index.tools {
        Some(tools) => with_tools_on_path(started.command, tools),
        None => started.command,
      },
      prompt: sources,
      mcp_file: started.mcp_file,
      left_out,
      code_index: index.used,
      app_server: started.app_server,
      claude_approvals,
      tool_relay: started.tool_relay,
      mcp_secrets,
      claude_exemptions,
      agent: body.role().as_str().to_owned(),
      memory: recalled.injected(),
      permissions: enforced.launch(),
    })
  }
}

/// `prompt` with the paragraph that points the agent at its Orchester tools when it is `told` of them: a new
/// session that gets them.
fn told_of_tools(prompt: String, tools: &ToolHost, told: bool) -> String {
  if told {
    format!(
      "{prompt}

{}",
      tools.tools().prompt_paragraph()
    )
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
