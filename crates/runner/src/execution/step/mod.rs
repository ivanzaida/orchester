//! Starting agent steps: which ready executions start (as agent slots allow), where each works
//! ([`super::workspace`]), its invocation record and its agent CLI (Codex or Claude Code) in its
//! worktree — a new session,
//! or the step's earlier session continued — handed to a [`worker::Worker`] of its own.

mod command;
mod end;
pub(super) mod worker;

use orchester_core::{
  connection::stream::{AgentStream, InvocationOutcome, RunnerProblem},
  conversation::{Message, latest_question},
  id::StepExecutionId,
  resource::ModelId,
  run::{Invocation, MessageOccasion, Plan, StepExecutor, StepFailure, StepStatus, execution_tokens},
  store::{ActivityStore, ConversationStore, RunStore},
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};
use tokio::sync::mpsc;

use self::worker::{Stop, Worker};
use super::{
  record::RecordError,
  run::{ExecutionError, RunTask},
  workspace::{Start, Workspace},
};
use crate::supervisor::Supervised;

/// How an execution's next invocation continues it: the CLI session it resumes, if its last
/// invocation reported one, when that invocation started, and the people's messages it delivers.
#[derive(Default)]
pub(super) struct Continuation {
  pub(super) session: Option<String>,
  pub(super) last_started: Option<Timestamp>,
  /// People's messages queued for the agent, oldest first: the invocation delivers them.
  pub(super) messages: Vec<Message>,
  pub(super) occasion: Occasion,
}

/// Why an invocation delivers people's messages now ([`MessageOccasion`], owning the question).
#[derive(Default)]
pub(super) enum Occasion {
  /// Before the step started, or while it was stopped.
  #[default]
  Written,
  /// Its agent finished its turn while they came in.
  AfterTurn,
  /// They answer the question its agent asked.
  Answer(String),
  /// A follow-up of a run that succeeded.
  FollowUp,
}

impl Occasion {
  pub(super) fn as_prompt(&self) -> MessageOccasion<'_> {
    match self {
      Self::Written => MessageOccasion::Written,
      Self::AfterTurn => MessageOccasion::AfterTurn,
      Self::Answer(question) => MessageOccasion::Answer { question },
      Self::FollowUp => MessageOccasion::FollowUp,
    }
  }
}

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
  /// A tool step's command at work: it has no step budget and uses no tokens.
  pub(super) const fn command(
    stops: mpsc::UnboundedSender<Stop>,
    workspace: Workspace,
    invocation: Invocation,
  ) -> Self {
    Self {
      stops,
      workspace,
      invocation,
      budget: None,
      used: TokensUsed::ZERO,
      budget_reached: false,
      abandoned: false,
    }
  }

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
    self.answer_questions()?;
    // An agent stopped to take changed permissions while a pause was asked for waits for it.
    let continuing: Vec<StepExecutionId> = self
      .run
      .running_executions()
      .into_iter()
      .filter(|execution| !self.agents.contains_key(execution) && !self.run.pause_requested())
      .collect();
    for execution in continuing {
      // A plan revision works beside the others in a worktree of its own when anything else of
      // the run works or is ready: its first worktree was merged and removed with the plan.
      let start =
        if self.revises_plan(execution)? && (!self.agents.is_empty() || !self.run.ready_executions().is_empty()) {
          Start::BesideOthers
        } else {
          Start::Continues
        };
      if !self.start(execution, start)? {
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
    if self.is_tool_step(execution) {
      return self.start_command(execution, start);
    }
    let work = self.work(execution)?;
    if !self.board.take(self.run.id(), execution, work) {
      return Ok(false);
    }
    let workspace = Workspace::of(&self.root, &self.run, &self.worktree, execution, start);
    let mut continuation = self.begin_step(execution)?;
    let model = self.model(execution)?;
    let mut invocation = self
      .db
      .start_invocation(self.run.id(), execution, &model, Timestamp::now())?;
    if continuation.session.is_some() {
      invocation.session.clone_from(&continuation.session);
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
    let (command, provider, prompt) = match self.command(execution, &continuation, workspace.path()) {
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
    // Messages that never reached a started agent stay queued.
    self.deliver(execution, &invocation, std::mem::take(&mut continuation.messages))?;
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

  /// Starts `execution` if it has not started; a follow-up resumes the session of its step's passed
  /// execution. One that was stopped mid-step, that a reviewer sent back, that asked a question or
  /// that finished its turn while messages came in continues: the session of its last invocation,
  /// if the CLI reported one, is resumed. Either way the messages queued for its agent go with it.
  fn begin_step(&mut self, execution: StepExecutionId) -> Result<Continuation, ExecutionError> {
    let messages = self.queued_for(execution)?;
    let status = self.run.execution(execution).map(|execution| execution.status());
    if status == Some(StepStatus::Pending) {
      self.transition(|run, now| run.start_step(execution, now))?;
      let Some(followed) = self.run.followed_up(execution) else {
        return Ok(Continuation {
          messages,
          ..Continuation::default()
        });
      };
      return Ok(Continuation {
        session: self
          .last_invocation(followed)?
          .and_then(|invocation| invocation.session),
        last_started: None,
        messages,
        occasion: Occasion::FollowUp,
      });
    }
    let last = self.last_invocation(execution)?;
    let started = last.as_ref().map(|invocation| invocation.started_at);
    let conversation = self.db.messages(self.run.task())?;
    // Asked in the last invocation, so the one that starts now answers it.
    let question = latest_question(&conversation, execution)
      .filter(|question| started.is_none_or(|started| question.created_at() >= started))
      .map(|question| question.body().as_str().to_owned());
    let completed =
      last.as_ref().and_then(|invocation| invocation.outcome.as_ref()) == Some(&InvocationOutcome::Completed);
    let occasion = match question {
      Some(question) => Occasion::Answer(question),
      None if completed => Occasion::AfterTurn,
      None => Occasion::Written,
    };
    Ok(Continuation {
      last_started: started,
      session: last.and_then(|invocation| invocation.session),
      messages,
      occasion,
    })
  }

  /// The latest invocation of `execution`, if it had one.
  fn last_invocation(&self, execution: StepExecutionId) -> Result<Option<Invocation>, ExecutionError> {
    Ok(
      self
        .db
        .invocations(self.run.id())?
        .into_iter()
        .rev()
        .find(|invocation| invocation.execution == execution),
    )
  }

  /// Whether `execution` continues because a reviewer sent its plan back: its last invocation
  /// ended with the plan, so it starts over where the run's graph stands now, not where it worked.
  fn revises_plan(&self, execution: StepExecutionId) -> Result<bool, ExecutionError> {
    let plans = self.db.plans(self.run.id()).map_err(RecordError::from)?;
    let last_started = self.last_invocation(execution)?.map(|invocation| invocation.started_at);
    Ok(Plan::awaiting_revision(&plans, execution, last_started).is_some())
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

  /// The agent or command did not start: the problem is reported, the invocation ends and the step
  /// fails.
  pub(super) fn not_started(
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
