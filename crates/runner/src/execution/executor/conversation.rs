//! The task conversation inside an executing run (`docs/design/orchester-workspace.md`, "Task conversation"): the
//! run's agents take the messages queued for them at their invocation boundaries, acknowledge them as their turns
//! end, ask their questions, and a run that succeeded with messages queued is followed up. The project-wide side,
//! for runs no task executes, is `super::super::coordinator::conversation`'s.
//!
//! A message sent now goes to the agent at work it reaches while its turn runs
//! ([`RunTask::steer`]); one its turn ended before is read at the turn's end like any other.

use orchester_core::{
  attachment::input::AttachedFile,
  conversation::{self, AgentMessage, Arrival, Delivery, Message, MessageBody},
  id::{ResourceId, StepExecutionId},
  run::{Invocation, RunStatus, StepExecutor, StepFailure, StepOutcome, StepOutput},
  store::ConversationStore,
  time::Timestamp,
};
use orchester_ipc::Event;

use super::{
  attached::{self, AttachedError},
  control::{Ack, answer},
  link::messages as message_link,
  question::held_by_question,
  run::{ExecutionError, RunTask},
  step::Active,
};

impl RunTask {
  /// A person's messages were stored: the ones sent now go to the agents at work they reach, the
  /// others are picked up as the run's agents reach a boundary.
  pub(super) fn deliver_messages(&mut self, ack: Ack) -> Result<(), ExecutionError> {
    answer(ack, Ok(()));
    self.steer()
  }

  /// Hands each message sent now and still queued to the first agent at work it reaches, once.
  pub(super) fn steer(&mut self) -> Result<(), ExecutionError> {
    let mut executions: Vec<StepExecutionId> = self
      .agents
      .iter()
      .filter(|(_, active)| active.steers.is_some() && !active.abandoned)
      .map(|(execution, _)| *execution)
      .collect();
    if executions.is_empty() {
      return Ok(());
    }
    executions.sort_unstable();
    let messages = message_link::messages(&*self.link, self.run.task())?;
    for execution in executions {
      let Some(profile) = self.profile_of(execution) else {
        continue;
      };
      let steered: Vec<Message> = messages
        .iter()
        .filter(|message| message.steers(self.run.id(), profile) && !self.steered.contains_key(&message.id()))
        .cloned()
        .collect();
      if steered.is_empty() {
        continue;
      }
      self
        .steered
        .extend(steered.iter().map(|message| (message.id(), execution)));
      tracing::info!(run = %self.run.id(), %execution, count = steered.len(), "messages sent now go to the agent");
      if let Some(steers) = self.agents.get(&execution).and_then(|active| active.steers.as_ref())
        && steers.send(steered).is_err()
      {
        // The agent has just ended: its end reads what is still queued.
        tracing::debug!(%execution, "the agent ended before the messages sent now");
      }
    }
    Ok(())
  }

  /// The agent profile `execution`'s step runs on; `None` for a tool step.
  pub(super) fn profile_of(&self, execution: StepExecutionId) -> Option<ResourceId> {
    self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().step(execution.step()))
      .and_then(|step| match step.executor() {
        StepExecutor::Agent { profile, .. } => Some(profile.resource),
        StepExecutor::Tool { .. } => None,
      })
  }

  /// People's messages queued for `execution`'s agent: to Orchestrator or to its profile, except
  /// the ones sent now that an agent at work has on their way to it.
  pub(super) fn queued_for(&self, execution: StepExecutionId) -> Result<Vec<Message>, ExecutionError> {
    let Some(profile) = self.profile_of(execution) else {
      return Ok(Vec::new());
    };
    let messages = message_link::messages(&*self.link, self.run.task())?;
    Ok(
      messages
        .into_iter()
        .filter(|message| message.reaches(self.run.id(), profile) && !self.steered.contains_key(&message.id()))
        .collect(),
    )
  }

  /// `execution`'s agent ended: the messages sent now that it had and did not take are queued for
  /// any agent again, its own end first.
  pub(super) fn release_steered(&mut self, execution: StepExecutionId) {
    self.steered.retain(|_, holder| *holder != execution);
  }

  /// The files people attached to `messages`, in their order, as an agent that reads them is given them: asked
  /// through the link and kept in the executor's attachments folder ([`attached`]).
  pub(super) fn attached_to(&self, messages: &[Message]) -> Result<Vec<AttachedFile>, AttachedError> {
    let folder = self.execution.attachments();
    let mut files = Vec::new();
    for message in messages {
      let contents = message_link::attached(&*self.link, message.id())?;
      files.extend(attached::kept_in(&folder, contents)?);
    }
    Ok(files)
  }

  /// `messages` went to `execution`'s agent with `invocation`, which started with them, as
  /// `arrival` says.
  pub(super) fn deliver(
    &mut self,
    execution: StepExecutionId,
    invocation: &Invocation,
    mut messages: Vec<Message>,
    arrival: Arrival,
  ) -> Result<(), ExecutionError> {
    if messages.is_empty() {
      return Ok(());
    }
    for message in &mut messages {
      message
        .deliver(
          self.run.id(),
          (execution, invocation.id),
          invocation.started_at,
          arrival,
        )
        .map_err(ExecutionError::Conversation)?;
    }
    self.db.save_deliveries(&messages)?;
    tracing::info!(run = %self.run.id(), %execution, count = messages.len(), "messages delivered");
    self.announce();
    Ok(())
  }

  /// `execution`'s agent ended a turn with `output`: the messages delivered to it are acknowledged,
  /// and its final message is its reply — unless it asked a question, which is its message instead.
  pub(super) fn acknowledge(
    &mut self,
    execution: StepExecutionId,
    invocation: &Invocation,
    output: &StepOutput,
  ) -> Result<(), ExecutionError> {
    let now = Timestamp::now();
    let mut acknowledged: Vec<Message> = message_link::messages(&*self.link, self.run.task())?
      .into_iter()
      .filter(
        |message| matches!(message.delivery(), Some(Delivery::Delivered { execution: to, .. }) if to == execution),
      )
      .collect();
    if acknowledged.is_empty() {
      return Ok(());
    }
    for message in &mut acknowledged {
      message.acknowledge(now).map_err(ExecutionError::Conversation)?;
    }
    self.db.save_deliveries(&acknowledged)?;
    if output.outcome != Some(StepOutcome::Question)
      && let Ok(body) = MessageBody::new(&output.text)
    {
      let reply = Message::from_agent(
        self.run.task(),
        self.run.id(),
        (execution, invocation.id),
        AgentMessage::Reply,
        body,
        now,
      );
      self.db.add_agent_message(&reply)?;
    }
    self.announce();
    Ok(())
  }

  /// `execution`'s agent asked `question`: its work is committed, its question is its message to the
  /// person, and its step waits for the answer. A question that says nothing fails the step.
  pub(super) fn ask(
    &mut self,
    execution: StepExecutionId,
    active: &Active,
    question: &str,
  ) -> Result<(), ExecutionError> {
    self.save(execution, active, "saved with a question")?;
    let Ok(body) = MessageBody::new(question) else {
      return self.fail(execution, StepFailure::AgentFailed);
    };
    let message = Message::from_agent(
      self.run.task(),
      self.run.id(),
      (execution, active.invocation.id),
      AgentMessage::Question,
      body,
      Timestamp::now(),
    );
    self.db.add_agent_message(&message)?;
    self.transition(|run, now| run.wait_step(execution, orchester_core::run::WaitReason::Message, now))?;
    let agent = self.work(execution)?.map(|work| work.agent).unwrap_or_default();
    tracing::info!(run = %self.run.id(), %execution, "question asked");
    self.notifier.send(Event::QuestionAsked {
      run: self.run.id(),
      task: self.run.task(),
      task_key: self.task.key().to_string(),
      agent,
    });
    Ok(())
  }

  /// Resumes the steps whose agent asked a question that people's messages now answer, unless the
  /// run is to pause. A question with suggested answers that still waits holds its step: the messages
  /// reach the agent with its answer.
  pub(super) fn answer_questions(&mut self) -> Result<(), ExecutionError> {
    if self.run.pause_requested() {
      return Ok(());
    }
    for execution in self.run.awaiting_answer() {
      if !self.queued_for(execution)?.is_empty() && !held_by_question(&*self.link, self.run.id(), execution)? {
        self.transition(|run, now| run.resume_step(execution, now))?;
      }
    }
    Ok(())
  }

  /// Follows up a run that just succeeded while people's messages were queued for its agents: the
  /// person asked, so the agent is asked. Not after a follow-up that failed, which would fail again
  /// for the same reason.
  pub(super) fn follow_up_queued(&mut self) -> Result<bool, ExecutionError> {
    let succeeded = self.run.status() == &RunStatus::Succeeded
      && self
        .run
        .current_attempt()
        .is_some_and(|attempt| attempt.status() == orchester_core::run::AttemptStatus::Succeeded);
    if !succeeded || self.board.is_stopping() {
      return Ok(false);
    }
    let messages = message_link::messages(&*self.link, self.run.task())?;
    let Some(step) = conversation::follow_up_step(&self.run, &messages) else {
      return Ok(false);
    };
    self.transition(|run, now| run.follow_up(&step, now))?;
    // The run succeeded a moment ago, which settled it; it runs again.
    self.executes_again();
    tracing::info!(run = %self.run.id(), %step, "follow-up started for queued messages");
    Ok(true)
  }

  fn announce(&self) {
    self.notifier.send(Event::RunChanged {
      run: self.run.id(),
      task: self.run.task(),
    });
  }
}
