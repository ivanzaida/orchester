//! What running a call takes, the executor contract: a program with its arguments, environment, time
//! limit and how much of its output the agent gets ([`ToolProgram`]), which the caller runs with its own
//! process runner and hands back to [`ToolProgram::finished`]; or a result already known, such as a read
//! outside the worktree, which happens here.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
  time::Duration,
};

use super::{OutputKept, OutsideReads, ProgramOutput, ToolCall, ToolCallError, ToolResult, git};
use crate::{
  command::{Command, CommandError},
  connection::invocation::{Platform, agent_environment},
  resource::{CommandAction, CommandEnvironment, ExitCodes},
};

/// A command tool of the project's Tools library offered to the step, as the run pinned it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectTool {
  name: String,
  description: String,
  action: CommandAction,
}

impl ProjectTool {
  pub fn new(name: &str, description: &str, action: CommandAction) -> Self {
    Self {
      name: name.to_owned(),
      description: description.to_owned(),
      action,
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn description(&self) -> &str {
    &self.description
  }
}

/// Where and with what the calls of one step run.
#[derive(Clone, Copy, Debug)]
pub struct ToolContext<'a> {
  pub worktree: &'a Path,
  /// The environment Orchester runs in; programs get the part an agent CLI gets
  /// ([`agent_environment`]), as the agent's own commands do.
  pub inherited: &'a [(OsString, OsString)],
  /// The variables the project adds for every program its runs start.
  pub project: &'a CommandEnvironment,
  pub outside: &'a OutsideReads,
  pub project_tools: &'a [ProjectTool],
  pub platform: Platform,
  /// The worktree's own git folder, which git is pinned to: a `.git` file the agent rewrote cannot send git
  /// to a folder with a configuration of its making.
  pub git_dir: &'a Path,
  /// An empty folder the runner owns, which git takes as its hooks folder, so no hook runs.
  pub hooks: &'a Path,
}

impl ToolContext<'_> {
  /// The variables a program of a call starts with: the agent's part of the inherited environment,
  /// then the project's.
  pub(super) fn variables(&self) -> Vec<(String, String)> {
    let mut variables = agent_environment(self.inherited.iter().cloned());
    self.project.add_to(&mut variables);
    variables
  }
}

/// A program a call runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolProgram {
  command: Command,
  timeout: Duration,
  limit: usize,
  kept: OutputKept,
  expected: ExitCodes,
}

impl ToolProgram {
  pub(super) fn new(command: Command, timeout: Duration, (limit, kept): (usize, OutputKept)) -> Self {
    Self {
      command,
      timeout,
      limit,
      kept,
      expected: ExitCodes::success(),
    }
  }

  #[must_use]
  fn expecting(mut self, codes: ExitCodes) -> Self {
    self.expected = codes;
    self
  }

  pub const fn command(&self) -> &Command {
    &self.command
  }

  /// How long the program may run before it is stopped.
  pub const fn timeout(&self) -> Duration {
    self.timeout
  }

  /// How much of each stream the runner keeps, and which end: it reads them as they come and holds no more.
  pub const fn cap(&self) -> (usize, OutputKept) {
    (self.limit, self.kept)
  }

  /// What the agent is answered once the program ended as `outcome` says.
  pub fn finished(&self, outcome: Result<ProgramOutput, CommandError>) -> ToolResult {
    ToolResult::of_program(outcome, &self.expected, (self.limit, self.kept), self.timeout)
  }
}

/// What running a call takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolWork {
  /// The caller runs this program and answers with [`ToolProgram::finished`].
  Run(Box<ToolProgram>),
  /// The answer, already known.
  Done(ToolResult),
}

impl ToolCall {
  /// What running this call in `context` takes, or why it does not run. A read outside the worktree
  /// happens here; it blocks on the file system.
  pub fn work(&self, context: &ToolContext<'_>) -> Result<ToolWork, ToolCallError> {
    match self {
      Self::ReadOutsideFile { path, offset, limit } => {
        let text = context.outside.read(path, *offset, *limit)?;
        Ok(ToolWork::Done(ToolResult::shown(text)))
      }
      Self::RunProjectTool { name } => Ok(ToolWork::Run(Box::new(project_program(name, context)?))),
      Self::Git(call) => Ok(ToolWork::Run(Box::new(git::program(call, context)))),
    }
  }
}

/// The program of project tool `name`, found as a tool step finds it.
fn project_program(name: &str, context: &ToolContext<'_>) -> Result<ToolProgram, ToolCallError> {
  let tool = context
    .project_tools
    .iter()
    .find(|tool| tool.name == name)
    .ok_or_else(|| ToolCallError::UnknownProjectTool(name.to_owned()))?;
  let action = &tool.action;
  let inherited_path = context
    .inherited
    .iter()
    .find(|(variable, _)| variable.to_string_lossy().eq_ignore_ascii_case("PATH"))
    .map(|(_, value)| value.clone());
  let path = action.search_path(context.project, inherited_path);
  let program: PathBuf = action
    .program_candidates(context.worktree, path.as_deref(), context.platform)
    .into_iter()
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| ToolCallError::ProgramNotFound(action.program().as_str().to_owned()))?;
  let command = action.process(
    &program,
    context.worktree,
    context.inherited.iter().cloned(),
    context.project,
  );
  let limit = (action.output_limit().bytes(), OutputKept::Tail);
  Ok(ToolProgram::new(command, action.timeout().duration(), limit).expecting(action.exit_codes().clone()))
}
