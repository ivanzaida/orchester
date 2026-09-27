//! Starting agent steps: which ready executions start (as agent slots allow), where each works
//! ([`super::workspace`]), its invocation record and its agent CLI (Codex or Claude Code) in its
//! worktree — a new session,
//! or the step's earlier session continued — handed to a [`worker::Worker`] of its own.

mod command;
mod end;
pub(super) mod worker;

use orchester_core::{
  connection::stream::{AgentStream, InvocationOutcome, RunnerProblem},
  id::StepExecutionId,
  resource::ModelId,
  run::{Invocation, StepExecutor, StepFailure, StepStatus, execution_tokens},
  store::ActivityStore,
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};
use tokio::sync::mpsc;

use self::worker::{Stop, Worker};
use super::{
  run::{ExecutionError, RunTask},
  workspace::{Start, Workspace},
};
use crate::supervisor::Supervised;

/// An agent at work on one execution of the run.
pub(super) struct Active {
  stops: mpsc::UnboundedSender<Stop>,
  pub(super) workspace: Workspace,
  /// The invocation as it started, for reporting problems around it.
  pub(super) invocation: Invocation,
  /// The step's budget and what the execution used of it, over all its invocations.
  budget: Option<TokenBudget>,
  used: TokensUsed,
  budget_reached: bool,
  /// Another step failed and ended the attempt; this agent is being ended and its end changes
  /// nothing.
  pub(super) abandoned: bool,
}

impl Active {
  pub(super) fn tell(&self, stop: Stop) {
    // The agent may have exited already; its end is on its way to the run.
    if self.stops.send(stop).is_err() {
      tracing::debug!(execution = %self.invocation.execution, "the agent has already ended");
    }
  }

  /// Counts `tokens` toward the step's budget; `true` once when they use it up.
  pub(super) fn count(&mut self, tokens: TokensUsed) -> bool {
    self.used += tokens;
    let reached = self.budget.is_some_and(|budget| self.used.reached(budget));
    let first = reached && !self.budget_reached;
    self.budget_reached |= reached;
    first
  }

  pub(super) fn abandon(&mut self) {
    self.abandoned = true;
    self.tell(Stop::Now(worker::StopCause::Abandoned));
  }

  pub(super) const fn budget(&self) -> Option<TokenBudget> {
    self.budget
  }
}

impl RunTask {
  /// Starts executions stopped mid-step, then ready ones, while slots are free and nothing asks the
  /// run to stop. `true` when an execution could run but no agent slot is free.
  pub(super) fn start_ready(&mut self) -> Result<bool, ExecutionError> {
    if !self.may_start() {
      return Ok(false);
    }
    let continuing: Vec<StepExecutionId> = self
      .run
      .running_executions()
      .into_iter()
      .filter(|execution| !self.agents.contains_key(execution))
      .collect();
    for execution in continuing {
      if !self.start(execution, Start::Continues)? {
        return Ok(true);
      }
    }
    let ready = self.run.executions_to_start(usize::MAX);
    let start = if self.agents.is_empty() && ready.len() == 1 {
      Start::Alone
    } else {
      Start::BesideOthers
    };
    for execution in ready {
      if !self.may_start() {
        return Ok(false);
      }
      // A step that failed to start may have ended the attempt the others belonged to.
      if self.run.execution(execution).map(|execution| execution.status()) != Some(StepStatus::Pending) {
        continue;
      }
      if !self.start(execution, start)? {
        return Ok(true);
      }
    }
    Ok(false)
  }

  /// Starts `execution`'s agent in a slot of its own; `false` when no slot is free. An agent that
  /// cannot start fails its step.
  fn start(&mut self, execution: StepExecutionId, start: Start) -> Result<bool, ExecutionError> {
    let work = self.work(execution)?;
    if !self.board.take(self.run.id(), execution, work) {
      return Ok(false);
    }
    let workspace = Workspace::of(&self.root, &self.run, &self.worktree, execution, start);
    let resume = self.begin_step(execution)?;
    let model = self.model(execution)?;
    let mut invocation = self
      .db
      .start_invocation(self.run.id(), execution, &model, Timestamp::now())?;
    if resume.is_some() {
      invocation.session.clone_from(&resume);
      self.db.save_invocation(&invocation)?;
    }
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
    let (command, provider, prompt) = match self.command(execution, resume.as_deref(), workspace.path()) {
      Ok(started) => started,
      Err(problem) => {
        self.not_started(execution, &mut invocation, problem, StepFailure::NotStarted)?;
        return Ok(true);
      }
    };
    self.db.save_invocation_prompt(invocation.id, &prompt)?;
    let process = match Supervised::start(&command) {
      Ok(process) => process,
      Err(error) => {
        let problem = RunnerProblem::CouldNotStart {
          detail: error.to_string(),
        };
        self.not_started(execution, &mut invocation, problem, StepFailure::NotStarted)?;
        return Ok(true);
      }
    };
    self.spawn(execution, invocation, AgentStream::new(provider), process, workspace)?;
    Ok(true)
  }

  fn spawn(
    &mut self,
    execution: StepExecutionId,
    invocation: Invocation,
    output: AgentStream,
    process: Supervised,
    workspace: Workspace,
  ) -> Result<(), ExecutionError> {
    let used = execution_tokens(&self.db.invocations(self.run.id())?, execution);
    let (stops, stopped) = mpsc::unbounded_channel();
    let worker = Worker {
      root: self.root.clone(),
      invocation: invocation.clone(),
      output,
      process,
      notifier: self.notifier.clone(),
      events: self.worker_events.clone(),
      stops: stopped,
    };
    tokio::spawn(worker.work());
    // The budget in force: one raised for the run after it stopped at the step's limit, else the
    // pipeline's.
    let budget = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.step_budget(execution.step()));
    self.agents.insert(
      execution,
      Active {
        stops,
        workspace,
        invocation,
        budget,
        used,
        budget_reached: false,
        abandoned: false,
      },
    );
    Ok(())
  }

  /// Starts `execution` if it has not started. One that was stopped mid-step continues: the
  /// session of its last invocation, if the CLI reported one, is resumed.
  fn begin_step(&mut self, execution: StepExecutionId) -> Result<Option<String>, ExecutionError> {
    let status = self.run.execution(execution).map(|execution| execution.status());
    if status == Some(StepStatus::Pending) {
      self.transition(|run, now| run.start_step(execution, now))?;
      return Ok(None);
    }
    Ok(
      self
        .db
        .invocations(self.run.id())?
        .into_iter()
        .rev()
        .find(|invocation| invocation.execution == execution)
        .and_then(|invocation| invocation.session),
    )
  }

  fn model(&self, execution: StepExecutionId) -> Result<ModelId, ExecutionError> {
    let step = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().step(execution.step()));
    match step.map(|step| step.executor()) {
      Some(StepExecutor::Agent { model, .. }) => Ok(model.clone()),
      _ => Err(ExecutionError::NotAnAgentStep(execution)),
    }
  }

  /// The agent did not start: the problem is reported, the invocation ends and the step fails.
  fn not_started(
    &mut self,
    execution: StepExecutionId,
    invocation: &mut Invocation,
    problem: RunnerProblem,
    failure: StepFailure,
  ) -> Result<(), ExecutionError> {
    self.report(invocation, problem)?;
    invocation.ended_at = Some(Timestamp::now());
    invocation.outcome = Some(InvocationOutcome::NotStarted);
    self.db.save_invocation(invocation)?;
    self.board.release(execution);
    self.fail(execution, failure)
  }
}
