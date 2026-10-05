//! The step's Orchester tools (`orchester_core::orchester_tools`), served over the invocation's relay
//! ([`super::relay`]) and run by the runner: git in the worktree, a read outside it, and the project's
//! command tools the run pinned for this step. A call runs off the async workers, in the worktree, with
//! the environment the agent's own programs get; its answer has the invocation's secret values removed.
//! Whether a call asks was decided before it arrives: the CLI asked a person first for a tool that asks
//! (Claude Code's permission prompt, Codex's approval request), as for every MCP tool.
//!
//! Codex gets the relay as an MCP server of its own configuration (`--orchester-tools`, without the
//! permission prompt); Claude Code gets it with its permission prompt ([`super::claude_approval`]).

use std::{
  ffi::OsString,
  io,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  command::{CommandRunner, SecretVariable},
  connection::{
    claude_approval::ApprovalToken,
    invocation::{CodexOrchester, Platform},
    stream::RunnerProblem,
  },
  id::InvocationId,
  orchester_tools::{OutsideReads, ProjectTool, StepTools, ToolCall, ToolContext, ToolResult, ToolWork},
  permission::{AgentPermissions, category::CommandScope},
  resource::{CommandEnvironment, StepKey, Tool, ToolAction},
  store::ResourceStore,
};
use orchester_exec::ProcessRunner;
use serde_json::Value;

use super::{
  super::run::RunTask,
  claude_approval::ClaudeApprovals,
  mcp::McpSetup,
  relay::{Listening, Served, bind},
  start::problem,
};
use crate::{approval_relay::TOOLS_FLAG, private_file};

/// The repository's folder of Orchester's own files: runs' worktrees, the database, memory.
const STATE_DIR: &str = ".orchester";

/// What the calls of one step run with.
pub(in super::super) struct ToolHost {
  tools: StepTools,
  worktree: PathBuf,
  inherited: Vec<(OsString, OsString)>,
  project: CommandEnvironment,
  outside: OutsideReads,
  project_tools: Vec<ProjectTool>,
  /// The invocation's secret values, removed from every answer.
  secrets: Vec<SecretVariable>,
}

impl ToolHost {
  pub(super) const fn tools(&self) -> &StepTools {
    &self.tools
  }

  /// The tools of the server's `tools/list` result.
  pub(super) fn listed(&self) -> Vec<Value> {
    self.tools.mcp_tools()
  }

  /// This host, also removing `secrets` from its answers.
  #[must_use]
  pub(super) fn with_secrets(mut self, secrets: Vec<SecretVariable>) -> Self {
    self.secrets.extend(secrets);
    self
  }

  /// The answer to a call of tool `name` with `arguments`.
  pub(super) async fn call(self: &Arc<Self>, name: &str, arguments: &Value) -> ToolResult {
    let call = match ToolCall::parse(name, arguments, &self.tools) {
      Ok(call) => call,
      Err(error) => return ToolResult::refused(&error),
    };
    let host = Arc::clone(self);
    let answer = tokio::task::spawn_blocking(move || host.run(&call)).await;
    match answer {
      Ok(result) => result.redacted(&self.secrets),
      Err(error) => {
        tracing::warn!(%error, tool = name, "an Orchester tool call ended without an answer");
        ToolResult::unavailable("the call ended without an answer")
      }
    }
  }

  /// Runs `call`, blocking.
  fn run(&self, call: &ToolCall) -> ToolResult {
    let context = ToolContext {
      worktree: &self.worktree,
      inherited: &self.inherited,
      project: &self.project,
      outside: &self.outside,
      project_tools: &self.project_tools,
      platform: Platform::current(),
    };
    match call.work(&context) {
      Ok(ToolWork::Done(result)) => result,
      Ok(ToolWork::Run(program)) => program.finished(ProcessRunner::new(program.timeout()).run(program.command())),
      Err(error) => ToolResult::refused(&error),
    }
  }
}

/// The relay Codex starts as Orchester's server, its tools' approval modes, and its listener, ended with
/// the session.
pub(in super::super) struct CodexTools {
  pub(in super::super) server: CodexOrchester,
  /// The server's table in the thread's `mcp_servers` configuration.
  pub(in super::super) table: Value,
  /// Held so the listener lives as long as the CLI.
  _listening: Listening,
}

impl RunTask {
  /// The Orchester tools of `step` with `permissions`, working in `worktree` with the project's
  /// `environment` and the extra writable folders `writable`.
  fn tool_host(
    &self,
    (step, permissions): (&StepKey, &AgentPermissions),
    (worktree, writable): (&Path, &[PathBuf]),
    environment: &CommandEnvironment,
  ) -> Result<ToolHost, RunnerProblem> {
    let project_tools = self.offered_project_tools(step)?;
    let refused = [private_file::directory(self.project.id()), self.root.join(STATE_DIR)];
    Ok(ToolHost {
      tools: StepTools::for_step(permissions, &project_tools),
      worktree: worktree.to_path_buf(),
      inherited: std::env::vars_os().collect(),
      project: environment.clone(),
      outside: OutsideReads::new(CommandScope::on_host(worktree, writable), &refused),
      project_tools,
      secrets: Vec::new(),
    })
  }

  /// The Orchester tools of `step` with `permissions` and the MCP servers of `setup`, for invocation
  /// `invocation` working in `worktree` with the extra writable folders `writable` and the project's
  /// `environment`; and Claude Code's permission prompt, whose relay serves them to a Claude Code agent.
  pub(super) fn step_tools(
    &self,
    (step, permissions, setup): (&StepKey, &AgentPermissions, &McpSetup),
    (invocation, worktree, writable): (InvocationId, &Path, &[PathBuf]),
    environment: &CommandEnvironment,
  ) -> Result<(Arc<ToolHost>, Option<ClaudeApprovals>), RunnerProblem> {
    let host = self.tool_host((step, permissions), (worktree, writable), environment)?;
    let tools = Arc::new(host.with_secrets(setup.secrets()));
    let approvals = self.claude_approvals((permissions, setup, &tools), (invocation, worktree))?;
    Ok((tools, approvals))
  }

  /// The command tools the run pinned whose access rules let `step` of the run's pipeline call them.
  fn offered_project_tools(&self, step: &StepKey) -> Result<Vec<ProjectTool>, RunnerProblem> {
    let pipeline = self.run.snapshot().pipeline().resource;
    let read = |error: &dyn std::error::Error| problem(error.to_string());
    let mut offered = Vec::new();
    for pinned in self.run.snapshot().tools() {
      let Some(version) = self
        .db
        .version::<Tool>(pinned.resource, pinned.number)
        .map_err(|error| read(&error))?
      else {
        continue;
      };
      let body = version.body();
      let (ToolAction::Command(action), Some(access)) = (body.action(), body.access()) else {
        continue;
      };
      if !access.allows(pipeline, step) {
        continue;
      }
      let Some(resource) = self.db.resource(pinned.resource).map_err(|error| read(&error))? else {
        continue;
      };
      offered.push(ProjectTool::new(
        resource.name().as_str(),
        body.description(),
        action.clone(),
      ));
    }
    Ok(offered)
  }

  /// Orchester's server for a Codex invocation serving `tools`: the relay Codex starts and the listener it
  /// reaches.
  pub(super) fn codex_tools(&self, tools: Arc<ToolHost>) -> Result<CodexTools, RunnerProblem> {
    let relay = self
      .relay
      .as_deref()
      .ok_or_else(|| problem("Orchester cannot find its runner program, which serves Orchester's tools".to_owned()))?;
    let program = relay
      .to_str()
      .ok_or_else(|| problem("the runner's path is not valid Unicode".to_owned()))?;
    let bound = bind().map_err(|error: io::Error| problem(format!("cannot open Orchester's tools: {error}")))?;
    let token = ApprovalToken::generate();
    let server = CodexOrchester::new(program, &[TOOLS_FLAG], bound.address(), token.variable());
    let table = tools.tools().codex_tools();
    let listening = bound.serve(Served { prompt: None, tools }, token);
    Ok(CodexTools {
      server,
      table,
      _listening: listening,
    })
  }
}
