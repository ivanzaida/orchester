//! Starting agent steps: which ready executions start (as agent slots allow), where each works
//! ([`super::workspace`]), its invocation record and its agent (Codex, Claude Code, or a native agent,
//! [`native`]) in its worktree — a new session, or the step's earlier session continued — handed to a
//! [`worker::Worker`] of its own.

mod accepted;
mod answer;
pub(super) mod app_server;
mod approval;
mod base_notice;
mod checks;
mod claude_approval;
mod cli_version;
mod command_scope;
mod end;
mod mcp;
mod memory;
mod native;
mod orchester_tools;
mod qa_app;
pub(super) mod question;
mod relay;
mod relay_calls;
mod run_app;
mod run_records;
mod skills;
mod start;
mod stderr;
mod tool_files;
pub(super) mod worker;

use orchester_core::{
  command::SecretVariable,
  connection::stream::{AgentEvent, InvocationOutcome, RunnerProblem},
  conversation::{Arrival, Message, latest_question},
  id::StepExecutionId,
  memory::MemoryStore,
  permission::LaunchPermissions,
  resource::ModelId,
  revision::Revision,
  run::{EditProgress, Invocation, MessageOccasion, Plan, StepExecutor, StepFailure, StepStatus, execution_tokens},
  store::{ActivityStore, ConversationStore, RunStore, SettingsStore},
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};
use orchester_ipc::Event;
use tokio::sync::mpsc;

pub(super) use self::qa_app::QaApps;
pub use self::qa_app::QaSlots;
use self::{
  command_scope::command_scope,
  end::Taken,
  start::Driven,
  worker::{Stop, Worker},
};
use super::{
  record::RecordError,
  run::{ExecutionError, RunTask},
  workspace::{Start, Workspace},
};

/// What a started agent's worker gets besides its driver: the invocation's permissions, the secrets kept out of
/// its activity, the agent's role and the account its CLI runs on.
type AgentParts = (
  LaunchPermissions,
  Vec<SecretVariable>,
  (String, Option<orchester_core::id::AgentAccountId>),
);

/// How an execution's next invocation continues it: the CLI session it resumes, if its last
/// invocation reported one, when that invocation started, and the people's messages it delivers.
#[derive(Default)]
pub(super) struct Continuation {
  pub(super) session: Option<String>,
  pub(super) last_started: Option<Timestamp>,
  /// People's messages queued for the agent, oldest first: the invocation delivers them.
  pub(super) messages: Vec<Message>,
  pub(super) occasion: Occasion,
  /// The notice that the run's base branch moved, which the invocation delivers ([`base_notice`]).
  pub(super) notice: Option<orchester_core::base_move::BaseNotice>,
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
  /// A new message resumes a failed attempt with its earlier output as context.
  Recovery(String),
}

impl Occasion {
  /// How the messages an invocation started for this occasion reach its agent.
  const fn arrival(&self) -> Arrival {
    match self {
      Self::AfterTurn => Arrival::TurnEnd,
      Self::Written | Self::Answer(_) | Self::FollowUp | Self::Recovery(_) => Arrival::Prompt,
    }
  }

  pub(super) fn as_prompt(&self) -> MessageOccasion<'_> {
    match self {
      Self::Written => MessageOccasion::Written,
      Self::AfterTurn => MessageOccasion::AfterTurn,
      Self::Answer(question) => MessageOccasion::Answer { question },
      Self::FollowUp => MessageOccasion::FollowUp,
      Self::Recovery(previous_result) => MessageOccasion::Recovery { previous_result },
    }
  }
}

/// An agent at work on one execution of the run.
pub(super) struct Active {
  stops: mpsc::UnboundedSender<Stop>,
  pub(super) approvals: Option<mpsc::UnboundedSender<worker::ApprovalDecision>>,
  /// Where answers to a question the agent waits on in its turn go; `None` for a command tool.
  pub(super) questions: Option<mpsc::UnboundedSender<question::QuestionAnswer>>,
  /// Where people's messages sent now go to the agent; `None` for a command tool.
  pub(super) steers: Option<mpsc::UnboundedSender<Vec<Message>>>,
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
  /// The revision of the run's branch the agent started on, which its result is for; `None` when it could not be
  /// read, and for a command tool.
  pub(super) judged: Option<Revision>,
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
      approvals: None,
      questions: None,
      steers: None,
      workspace,
      invocation,
      budget: None,
      used: TokensUsed::ZERO,
      budget_reached: false,
      abandoned: false,
      judged: None,
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
    if self.passes_accepted(execution)? || self.waits_for_plan_reset(execution)? {
      return Ok(true);
    }
    let judged = match self.take_slot(execution)? {
      Taken::NoSlot => return Ok(false),
      Taken::Refused => return Ok(true),
      Taken::On(judged) => judged,
    };
    // A person's *Try now* holds until the agent has a slot: with every slot busy, the step tries again later.
    self.try_at_plan_limit.take_if(|asked| *asked == execution);
    let workspace = Workspace::of(&self.root, &self.run, &self.worktree, execution, start);
    let mut continuation = self.begin_step(execution)?;
    self.keep_session_on_its_account(execution, &mut continuation)?;
    continuation.notice = self.notice_for(execution)?;
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
    let start = self
      .agent_index()
      .map_err(|detail| RunnerProblem::CouldNotStart { detail })
      .and_then(|index| self.command(execution, &continuation, (invocation.id, workspace.path()), index));
    let mut start = match start {
      Ok(start) => start,
      Err(problem) => {
        self.not_started(execution, &mut invocation, problem, StepFailure::NotStarted)?;
        return Ok(true);
      }
    };
    self.db.save_invocation_permissions(invocation.id, &start.permissions)?;
    invocation.permissions = Some(start.permissions.clone());
    self.record_reached(invocation.id, &start)?;
    self.db.save_invocation_prompt(invocation.id, &start.prompt)?;
    self.db.save_invocation_code_index(invocation.id, &start.code_index)?;
    self.db.save_edit_progress(invocation.id, &EditProgress::started())?;
    self.db.save_injected_memory(invocation.id, &start.memory)?;
    self.report_unreadable_memory(&invocation)?;
    self.record_left_out(&invocation, std::mem::take(&mut start.left_out))?;
    let driven = match start.launch.start() {
      Ok(driven) => driven,
      Err(error) => {
        let problem = RunnerProblem::CouldNotStart {
          detail: error.to_string(),
        };
        self.not_started(execution, &mut invocation, problem, StepFailure::NotStarted)?;
        return Ok(true);
      }
    };
    // Messages and a notice that never reached a started agent stay queued.
    let arrival = continuation.occasion.arrival();
    self.deliver(
      execution,
      &invocation,
      std::mem::take(&mut continuation.messages),
      arrival,
    )?;
    self.deliver_notice(execution, &invocation, continuation.notice.take())?;
    let agent = (start.permissions, start.secrets, (start.agent, start.account));
    self.spawn(execution, invocation, (driven, agent), (workspace, judged))?;
    Ok(true)
  }

  /// The checks `execution`'s step is offered. Read once before for the agent's tools; should the read fail now,
  /// none: a check's call then asks each time and is never approved similar, the safe side.
  fn checks_of(&self, execution: StepExecutionId) -> Vec<orchester_core::orchester_tools::ProjectTool> {
    let Some(step) = self.run.execution(execution).map(|execution| execution.step().clone()) else {
      return Vec::new();
    };
    self.offered_checks(&step).unwrap_or_else(|problem| {
      tracing::warn!(?problem, "the step's checks could not be read for its approvals");
      Vec::new()
    })
  }

  /// Records what `invocation` reaches its model through and on: a native agent's engine, a CLI's account, or the API
  /// provider a CLI runs on.
  fn record_reached(
    &mut self,
    invocation: orchester_core::id::InvocationId,
    start: &start::AgentStart,
  ) -> Result<(), ExecutionError> {
    if let Some(engine) = &start.engine {
      self.db.save_invocation_engine(invocation, engine)?;
    }
    if let Some(account) = start.account {
      self.db.save_invocation_account(invocation, account)?;
    }
    if let Some((base_url, key)) = &start.provider {
      self.db.save_invocation_provider(invocation, base_url, *key)?;
    }
    Ok(())
  }

  /// Records what the agent of `invocation` works without: the MCP servers and the codebase index
  /// of `left_out`.
  fn record_left_out(&mut self, invocation: &Invocation, left_out: Vec<AgentEvent>) -> Result<(), ExecutionError> {
    if left_out.is_empty() {
      return Ok(());
    }
    let now = Timestamp::now();
    let events: Vec<(Timestamp, AgentEvent)> = left_out.into_iter().map(|event| (now, event)).collect();
    let sequence = self.db.append_activity(invocation, &events)?;
    self.notifier.send(Event::AgentActivity {
      run: self.run.id(),
      sequence,
    });
    Ok(())
  }

  fn spawn(
    &mut self,
    execution: StepExecutionId,
    invocation: Invocation,
    (driven, (permissions, secrets, (agent, account))): (Driven, AgentParts),
    (workspace, judged): (Workspace, Option<Revision>),
  ) -> Result<(), ExecutionError> {
    let (invocations, reviews) = (
      self.db.invocations(self.run.id())?,
      self.db.reviewer_usage(self.run.id())?,
    );
    let used = execution_tokens(&invocations, &reviews, execution);
    let (stops, stopped) = mpsc::unbounded_channel();
    let (approvals, decisions) = mpsc::unbounded_channel();
    let (questions, answers) = mpsc::unbounded_channel();
    let (steers, steered) = mpsc::unbounded_channel();
    let (reviews_held, reviews_ended) = mpsc::channel(1);
    let Driven {
      driver,
      mcp_file,
      claude_approvals,
      tool_relay,
      interactive,
      steered: takes_steers,
    } = driven;
    // A CLI's agent waits in its turn for an answer; a native agent's question ends its step instead.
    let asks_in_turn = !matches!(driver, worker::Driver::Native(_));
    let worker = Worker {
      scope: command_scope(
        &self.root,
        workspace.path(),
        &permissions,
        &self.db.project_environment()?,
      ),
      profile: self.profile_of(execution),
      checks: self.checks_of(execution),
      root: self.root.clone(),
      invocation: invocation.clone(),
      driver,
      notifier: self.notifier.clone(),
      events: self.worker_events.clone(),
      stops: stopped,
      mcp_file,
      claude_approvals,
      tool_relay,
      secrets,
      agent,
      account,
      task: self.task.id(),
      decisions,
      answers,
      steers: steered,
      reviewing: None,
      reviewer_keys: worker::ReviewerKeys {
        vault: self.vault.clone(),
        project: self.project.id(),
      },
      reviews_held,
      qa: std::sync::Arc::clone(&self.qa),
    };
    tokio::spawn(worker.work(reviews_ended));
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
        approvals: interactive.then_some(approvals),
        questions: asks_in_turn.then_some(questions),
        steers: takes_steers.then_some(steers),
        workspace,
        invocation,
        budget,
        used,
        budget_reached: false,
        abandoned: false,
        judged,
      },
    );
    Ok(())
  }

  /// Starts `execution` if it has not started; a follow-up resumes the session of its step's passed
  /// execution. One that was stopped mid-step, that a reviewer sent back, that asked a question or
  /// that finished its turn while messages came in continues its earlier session. Recovery of an
  /// upstream step starts dependent sessions afresh while retaining their saved work. Either way
  /// the messages queued for its agent go with it.
  fn begin_step(&mut self, execution: StepExecutionId) -> Result<Continuation, ExecutionError> {
    let messages = self.queued_for(execution)?;
    let status = self.run.execution(execution).map(|execution| execution.status());
    if status == Some(StepStatus::Pending) {
      self.transition(|run, now| run.start_step(execution, now))?;
      let recovered = self.run.recovered_execution(execution);
      let Some(followed) = recovered.or_else(|| self.run.followed_up(execution)) else {
        return Ok(Continuation {
          messages,
          ..Continuation::default()
        });
      };
      return Ok(Continuation {
        session: if recovered.is_some() {
          self.recovery_session(followed)?
        } else {
          self
            .last_invocation(followed)?
            .and_then(|invocation| invocation.session)
        },
        last_started: None,
        messages,
        occasion: if recovered.is_some() {
          let previous_result = self
            .db
            .step_outputs(self.run.id())?
            .into_iter()
            .find(|output| output.execution == followed)
            .map(|output| output.text)
            .unwrap_or_default();
          Occasion::Recovery(previous_result)
        } else {
          Occasion::FollowUp
        },
        notice: None,
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
      notice: None,
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

  fn recovery_session(&self, execution: StepExecutionId) -> Result<Option<String>, ExecutionError> {
    let invocations = self.db.invocations(self.run.id())?;
    let mut earlier = Some(execution);
    while let Some(execution) = earlier {
      if let Some(session) = invocations
        .iter()
        .rev()
        .filter(|invocation| invocation.execution == execution)
        .find_map(|invocation| invocation.session.clone())
      {
        return Ok(Some(session));
      }
      earlier = self.run.recovered_execution(execution);
    }
    Ok(None)
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
