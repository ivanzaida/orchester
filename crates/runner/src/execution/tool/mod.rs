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
  resource::{CommandAction, CommandEnvironment, Tool, ToolAction},
  run::{Invocation, StepExecutor, StepFailure, StepStatus, ToolCommand},
  store::{ActivityStore, ResourceStore},
  time::Timestamp,
};
use orchester_ipc::Event;
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
    let workspace = Workspace::of(&self.root, &self.run, &self.worktree, execution, start);
    if self.run.execution(execution).map(|execution| execution.status()) == Some(StepStatus::Pending) {
      self.transition(|run, now| run.start_step(execution, now))?;
    }
    let mut invocation = self
      .db
      .start_command_invocation(self.run.id(), execution, Timestamp::now())?;
    let workspace = match workspace {
      Ok(workspace) => workspace,
      Err(error) => {
        let problem = RunnerProblem::CheckpointFailed {
          detail: error.to_string(),
        };
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
    self.record(&invocation, started)?;
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
    let program =
      find_program(&command.action, &environment, worktree).ok_or_else(|| RunnerProblem::ProgramNotFound {
        program: command.action.program().as_str().to_owned(),
      })?;
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
    let version = self
      .db
      .version::<Tool>(pinned.resource, pinned.number)
      .map_err(|error| read(&error))?
      .ok_or_else(|| problem("the tool's pinned version no longer exists"))?;
    let ToolAction::Command(action) = version.body().action() else {
      return Err(problem("the tool is not a command tool"));
    };
    let tool = self
      .db
      .resource(pinned.resource)
      .map_err(|error| read(&error))?
      .map(|resource| resource.name().as_str().to_owned())
      .ok_or_else(|| problem("the tool no longer exists"))?;
    Ok(PinnedCommand {
      tool,
      action: action.clone(),
    })
  }

  /// Appends `event` of `invocation` to the transcript and announces it.
  fn record(
    &mut self,
    invocation: &Invocation,
    event: orchester_core::connection::stream::AgentEvent,
  ) -> Result<(), ExecutionError> {
    let sequence = self.db.append_activity(invocation, &[(Timestamp::now(), event)])?;
    self.notifier.send(Event::AgentActivity {
      run: self.run.id(),
      sequence,
    });
    Ok(())
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
      root: self.root.clone(),
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

/// The first file the program is, on the `PATH` it starts with or in `worktree`.
fn find_program(action: &CommandAction, project: &CommandEnvironment, worktree: &Path) -> Option<PathBuf> {
  let path = action.search_path(project, std::env::var_os("PATH"));
  action
    .program_candidates(worktree, path.as_deref(), Platform::current())
    .into_iter()
    .find(|candidate| candidate.is_file())
}
