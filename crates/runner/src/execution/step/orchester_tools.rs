//! The step's Orchester tools (`orchester_core::orchester_tools`), served over the invocation's relay
//! ([`super::relay`]) and run by the runner: the native agent's file tools ([`super::tool_files`]), git in
//! the worktree, pinned to its own git folder, a read outside it, and the step's skills ([`StepSkills`]). A call runs
//! off the async workers, in the worktree, with the environment the agent's own programs get; its output is read as it
//! comes and kept within its cap, and its answer has the invocation's secret values removed.
//!
//! A call of a tool that asks runs only once approved, which the runner holds itself: an approval given
//! through Claude Code's permission prompt leaves a grant for one call of that tool with exactly the
//! arguments the person saw ([`ToolHost::grant`]),
//! and a call that finds none is asked about by the runner before it runs ([`super::relay`]). Codex runs
//! every tool without asking, so the runner asks for it.
//!
//! The Tools library's command tools are offered to no agent: the library has no setting that offers a tool
//! to agents, and its access rules were made for pipeline steps (`docs/plan.md`, M8).

use std::{
  ffi::OsString,
  io,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  command::SecretVariable,
  connection::{
    claude_approval::ApprovalToken,
    invocation::{CodexOrchester, Platform},
    stream::RunnerProblem,
  },
  orchester_tools::{
    OrchesterTool, OutsideReads, StepSkills, StepTools, ToolCall, ToolCallError, ToolContext, ToolResult, ToolWork,
  },
  permission::{AgentPermissions, ResolvedFolder, category::CommandScope},
  resource::CommandEnvironment,
};
use orchester_exec::run_capped;
use parking_lot::Mutex;
use serde_json::Value;

use super::{
  super::run::RunTask,
  claude_approval::ClaudeApprovals,
  mcp::McpSetup,
  relay::{Listening, Served, bind},
  start::problem,
  tool_files::ToolFiles,
};
use crate::{approval_relay::TOOLS_FLAG, git::Git, private_file};

/// The folders the file tools of one step work in.
#[derive(Clone, Copy)]
pub(in super::super) struct ToolFolders<'a> {
  pub(in super::super) worktree: &'a Path,
  /// The extra writable folders the step is granted.
  pub(in super::super) writable: &'a [ResolvedFolder],
  /// The code index of the run's base revision, read-only, when the agent gets it.
  pub(in super::super) code_index: Option<&'a Path>,
  /// The step's skills, read-only, when it has any.
  pub(in super::super) skills: Option<&'a Path>,
}

impl<'a> ToolFolders<'a> {
  pub(in super::super) const fn new(
    worktree: &'a Path,
    writable: &'a [ResolvedFolder],
    (code_index, skills): (Option<&'a Path>, Option<&'a Path>),
  ) -> Self {
    Self {
      worktree,
      writable,
      code_index,
      skills,
    }
  }
}

/// The hooks folder git gets, empty, among the project's private files.
const NO_HOOKS: &str = "no-hooks";

/// What the calls of one step run with.
pub(in super::super) struct ToolHost {
  tools: StepTools,
  worktree: PathBuf,
  git_dir: PathBuf,
  hooks: PathBuf,
  inherited: Vec<(OsString, OsString)>,
  project: CommandEnvironment,
  outside: OutsideReads,
  /// The file tools, with the read stamps of this invocation.
  files: ToolFiles,
  /// The skills `list_skills` and `read_skill` serve.
  skills: StepSkills,
  /// The invocation's secret values, removed from every answer.
  secrets: Vec<SecretVariable>,
  /// Tools a person or a rule approved one call of, through the CLI's own prompt, not called yet.
  grants: Mutex<Vec<(OrchesterTool, Value)>>,
}

impl ToolHost {
  pub(super) const fn tools(&self) -> &StepTools {
    &self.tools
  }

  pub(super) fn secrets(&self) -> &[SecretVariable] {
    &self.secrets
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

  /// The call of tool `name` with `arguments`, or why it does not run.
  pub(super) fn parse(&self, name: &str, arguments: &Value) -> Result<ToolCall, ToolCallError> {
    ToolCall::parse(name, arguments, &self.tools)
  }

  /// Whether a call of `tool` needs an approval before it runs.
  pub(super) fn asks(&self, tool: OrchesterTool) -> bool {
    self.tools.asks(tool.as_str())
  }

  /// One approved call of `tool` with `arguments`, given through the CLI's own prompt.
  pub(super) fn grant(&self, tool: OrchesterTool, arguments: &Value) {
    self.grants.lock().push((tool, called_with(arguments)));
  }

  /// Takes the approval of one call of `tool` with exactly `arguments`; `false` when there is none.
  pub(super) fn take_grant(&self, tool: OrchesterTool, arguments: &Value) -> bool {
    let arguments = called_with(arguments);
    let mut grants = self.grants.lock();
    let Some(at) = grants
      .iter()
      .position(|(granted, with)| *granted == tool && *with == arguments)
    else {
      return false;
    };
    grants.remove(at);
    true
  }

  /// Runs `call`, approved or not needing an approval, off the async workers.
  pub(super) async fn run(self: &Arc<Self>, call: ToolCall) -> ToolResult {
    let host = Arc::clone(self);
    let tool = call.tool();
    match tokio::task::spawn_blocking(move || host.run_blocking(&call)).await {
      Ok(result) => result.redacted(&self.secrets),
      Err(error) => {
        tracing::warn!(%error, tool = tool.as_str(), "an Orchester tool call ended without an answer");
        ToolResult::unavailable("the call ended without an answer")
      }
    }
  }

  fn run_blocking(&self, call: &ToolCall) -> ToolResult {
    let context = ToolContext {
      worktree: &self.worktree,
      inherited: &self.inherited,
      project: &self.project,
      outside: &self.outside,
      project_tools: &[],
      platform: Platform::current(),
      git_dir: &self.git_dir,
      hooks: &self.hooks,
    };
    match call.work(&context) {
      Ok(ToolWork::Done(result)) => result,
      Ok(ToolWork::Files(input)) => self.files.run(&input, &context),
      Ok(ToolWork::Symbols(query)) => self.files.symbols(&query, &context),
      Ok(ToolWork::Skill(call)) => self.skills.run(&call),
      Ok(ToolWork::Run(program)) => program.finished(run_capped(program.command(), program.timeout(), program.cap())),
      Err(error) => ToolResult::refused(&error),
    }
  }
}

/// `arguments` as a call names them: none is an empty object.
fn called_with(arguments: &Value) -> Value {
  if arguments.is_null() {
    Value::Object(serde_json::Map::new())
  } else {
    arguments.clone()
  }
}

/// The relay Codex starts as Orchester's server, with its tools approved, and the listener it reaches.
pub(in super::super) struct CodexTools {
  pub(in super::super) server: CodexOrchester,
  pub(in super::super) listening: Listening,
}

impl RunTask {
  /// The Orchester tools of a step with `permissions`, the MCP servers of `setup` and `skills`, for invocation
  /// `invocation` working in `worktree` with the extra writable folders `writable` and the project's
  /// `environment`; and Claude Code's permission prompt, whose relay serves them to a Claude Code agent.
  pub(super) fn step_tools(
    &self,
    (permissions, setup, skills): (&AgentPermissions, &McpSetup, StepSkills),
    (invocation, folders): (orchester_core::id::InvocationId, ToolFolders<'_>),
    environment: &CommandEnvironment,
  ) -> Result<(Arc<ToolHost>, Option<ClaudeApprovals>), RunnerProblem> {
    let worktree = folders.worktree;
    let host = self.tool_host((permissions, skills), folders, environment)?;
    let tools = Arc::new(host.with_secrets(setup.secrets()));
    let approvals = self.claude_approvals((permissions, setup, &tools), (invocation, worktree))?;
    Ok((tools, approvals))
  }

  /// The Orchester tools of a step with `permissions` that offers `skills`, working in `folders` with the
  /// project's `environment`.
  fn tool_host(
    &self,
    (permissions, skills): (&AgentPermissions, StepSkills),
    folders: ToolFolders<'_>,
    environment: &CommandEnvironment,
  ) -> Result<ToolHost, RunnerProblem> {
    let worktree = folders.worktree;
    let writable: Vec<PathBuf> = folders
      .writable
      .iter()
      .map(|folder| folder.path().to_path_buf())
      .collect();
    let writable = writable.as_slice();
    let hooks = private_file::empty_folder(self.project.id(), NO_HOOKS).map_err(|error| {
      problem(format!(
        "cannot make the empty hooks folder of Orchester's tools: {error}"
      ))
    })?;
    let mut refused = vec![private_file::root()];
    refused.extend(self.relay.as_deref().and_then(Path::parent).map(Path::to_path_buf));
    let files = ToolFiles::new(permissions.repository(), folders, self.run.base().as_str())?;
    let mut tools = StepTools::for_step(permissions, &[]);
    if files.indexed() {
      tools = tools.with_code_index();
    }
    if !skills.is_empty() {
      tools = tools.with_skills();
    }
    Ok(ToolHost {
      tools,
      worktree: worktree.to_path_buf(),
      git_dir: self.worktree_git_dir(worktree)?,
      hooks,
      inherited: std::env::vars_os().collect(),
      project: environment.clone(),
      outside: OutsideReads::new(CommandScope::on_host(worktree, writable), &refused),
      files,
      skills,
      secrets: Vec::new(),
      grants: Mutex::new(Vec::new()),
    })
  }

  /// The git folder of `worktree`, read from the repository rather than from the worktree's `.git` file,
  /// which the agent may have rewritten: `<common git folder>/worktrees/<worktree's name>`.
  fn worktree_git_dir(&self, worktree: &Path) -> Result<PathBuf, RunnerProblem> {
    let missing = || {
      problem(format!(
        "cannot find the git folder of the worktree {}",
        worktree.display()
      ))
    };
    let common = Git::new(&self.root)
      .stdout(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
      .map_err(|error| problem(format!("cannot read the repository's git folder: {error}")))?;
    let name = worktree.file_name().ok_or_else(missing)?;
    let git_dir = PathBuf::from(common.trim()).join("worktrees").join(name);
    // The folder must be this worktree's: its `gitdir` names the worktree's `.git`.
    let names = std::fs::read_to_string(git_dir.join("gitdir")).map_err(|_| missing())?;
    let same = |path: &Path| std::fs::canonicalize(path).ok();
    match (same(Path::new(names.trim())), same(&worktree.join(".git"))) {
      (Some(named), Some(own)) if named == own => Ok(git_dir),
      _ => Err(missing()),
    }
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
    let server = CodexOrchester::new(
      program,
      &[TOOLS_FLAG],
      bound.address(),
      token.variable(),
      &tools.tools().codex_approved(),
    );
    let listening = bound.serve(Served { prompt: None, tools }, token);
    Ok(CodexTools { server, listening })
  }
}
