//! Tool steps that run a command tool (M10a). A tool step works where an agent step would (see
//! `workspace`): alone in the run's worktree, beside other steps in a worktree of its own that is
//! merged back when it passes. Its program is found on the `PATH` or in the worktree and started
//! as the process runner starts every program (Windows command scripts included), with the cleaned
//! environment, the project's variables and the tool's, no console window and its whole process
//! tree ended with it. It takes a command slot instead of an agent slot (see `board`); its run is
//! an invocation of the step, so its time counts toward the run's time limit. What it printed is
//! kept up to the tool's output limit and recorded in the transcript when it ends; its exit code
//! decides the step, and a short summary is the step's output for the steps after it.

mod end;
mod worker;

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{invocation::Platform, stream::RunnerProblem},
  id::StepExecutionId,
  resource::{CommandAction, CommandEnvironment, ToolAction},
  run::{Invocation, StepExecutor, StepFailure, StepStatus, ToolCommand},
  time::Timestamp,
};
use orchester_store::InvocationStart;
use tokio::sync::mpsc;

pub(super) use self::worker::CommandFinished;
use self::worker::CommandWorker;
use super::{
  run::{ExecutionError, RunTask},
  step::Active,
  workspace::{Start, Workspace},
};
use crate::supervisor::Supervised;

/// The command tool a tool step runs, as the run pinned it.
struct PinnedCommand {
  tool: String,
  action: CommandAction,
}

impl RunTask {
  /// Whether `execution` belongs to a tool step.
  pub(super) fn is_tool_step(&self, execution: StepExecutionId) -> bool {
    self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().step(execution.step()))
      .is_some_and(|step| matches!(step.executor(), StepExecutor::Tool { .. }))
  }

  /// Starts `execution`'s command in a command slot; `false` when none is free. A command that
  /// cannot start fails its step.
  pub(super) fn start_command(&mut self, execution: StepExecutionId, start: Start) -> Result<bool, ExecutionError> {
    if !self.board.take_command(execution) {
      return Ok(false);
    }
    let workspace = match self.workspace(execution, start) {
      Ok(None) => {
        self.board.release(execution);
        return Ok(false);
      }
      Ok(Some(workspace)) => Ok(workspace),
      Err(detail) => Err(detail),
    };
    if self.run.execution(execution).map(|execution| execution.status()) == Some(StepStatus::Pending) {
      self.transition(|run, now| run.start_step(execution, now))?;
    }
    // The command checks the base the run will merge into (ORC-151).
    self.merge_moved_base(execution, workspace.as_ref().ok())?;
    let start = InvocationStart::command(self.run.id(), execution, Timestamp::now());
    let mut invocation = self.begin_invocation(&start)?;
    let workspace = match workspace {
      Ok(workspace) => workspace,
      Err(detail) => {
        let problem = RunnerProblem::CheckpointFailed { detail };
        self.not_started(execution, &mut invocation, problem, StepFailure::CheckpointFailed)?;
        return Ok(true);
      }
    };
    let (command, program, environment) = match self.resolve(execution, workspace.path()) {
      Ok(resolved) => resolved,
      Err(problem) => {
        self.not_started(execution, &mut invocation, problem, StepFailure::NotStarted)?;
        return Ok(true);
      }
    };
    let started = ToolCommand {
      tool: &command.tool,
      action: &command.action,
    }
    .started();
    self.record_activity(&invocation, vec![(Timestamp::now(), started)])?;
    let process = command.action.process(
      &program,
      workspace.path(),
      std::env::vars_os(),
      &environment,
      Platform::current(),
    );
    match Supervised::start(&process) {
      Ok(process) => self.follow(execution, invocation, command, process, workspace),
      Err(error) => {
        let problem = RunnerProblem::CouldNotStart {
          detail: error.to_string(),
        };
        self.not_started(execution, &mut invocation, problem, StepFailure::NotStarted)?;
      }
    }
    Ok(true)
  }

  /// The pinned command tool of `execution`'s step, the file its program is and the variables the
  /// project adds, or why it cannot start in `worktree`.
  fn resolve(
    &self,
    execution: StepExecutionId,
    worktree: &Path,
  ) -> Result<(PinnedCommand, PathBuf, CommandEnvironment), RunnerProblem> {
    let command = self.pinned_command(execution)?;
    let environment = self.environment()?;
    let program = find_program(&command.action, &environment, worktree, &self.root)?;
    Ok((command, program, environment))
  }

  /// The command tool of `execution`'s step, as the run pinned it, or why it cannot run.
  fn pinned_command(&self, execution: StepExecutionId) -> Result<PinnedCommand, RunnerProblem> {
    let problem = |detail: &str| RunnerProblem::CouldNotStart {
      detail: detail.to_owned(),
    };
    let pinned = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().step(execution.step()))
      .and_then(|step| match step.executor() {
        StepExecutor::Tool { tool } => Some(*tool),
        StepExecutor::Agent { .. } => None,
      })
      .ok_or_else(|| problem("the step is not a tool step"))?;
    let read = |error: &dyn std::error::Error| problem(&error.to_string());
    let body = self.resolver.tool(&*self.link, pinned).map_err(|error| read(&error))?;
    let ToolAction::Command(action) = body.action() else {
      return Err(problem("the tool is not a command tool"));
    };
    let (_, name) = self
      .resolver
      .label(&*self.link, pinned.resource, "tool")
      .map_err(|error| read(&error))?;
    Ok(PinnedCommand {
      tool: name.as_str().to_owned(),
      action: action.clone(),
    })
  }

  /// Hands the started program to a worker of its own.
  fn follow(
    &mut self,
    execution: StepExecutionId,
    invocation: Invocation,
    command: PinnedCommand,
    process: Supervised,
    workspace: Workspace,
  ) {
    let step = self
      .run
      .execution(execution)
      .expect("a started command belongs to an execution of the run")
      .step()
      .clone();
    let (stops, stopped) = mpsc::unbounded_channel();
    let worker = CommandWorker {
      sink: self.sink.another(),
      invocation: invocation.clone(),
      tool: command.tool,
      action: command.action,
      step,
      process,
      notifier: self.notifier.clone(),
      events: self.worker_events.clone(),
      stops: stopped,
    };
    tokio::spawn(worker.work());
    self
      .agents
      .insert(execution, Active::command(stops, workspace, invocation));
  }
}

/// The first file the program is, on the `PATH` it starts with or in `worktree`; a program stored as a fixed file
/// must still lead outside `repository` and stay on this machine (`CommandProgram::check_place`).
fn find_program(
  action: &CommandAction,
  project: &CommandEnvironment,
  worktree: &Path,
  repository: &Path,
) -> Result<PathBuf, RunnerProblem> {
  let program = || action.program().as_str().to_owned();
  let path = action.search_path(project, std::env::var_os("PATH"));
  let found = action
    .program_candidates(worktree, path.as_deref(), Platform::current())
    .into_iter()
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| RunnerProblem::ProgramNotFound { program: program() })?;
  action
    .program()
    .check_place(&found, repository)
    .map_err(|_| RunnerProblem::ProgramMoved { program: program() })?;
  Ok(found)
}
