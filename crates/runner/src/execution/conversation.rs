//! The task conversation at work (`docs/design/orchester-workspace.md`, "Task conversation"). A
//! person's messages, stored by the desktop, are delivered at the run's next invocation boundary:
//! every agent invocation that starts takes the messages queued for its agent into its prompt; an
//! agent that finishes its turn while messages for it are queued does not end its step but continues
//! its session to read them (after its work is committed, like Apply now); a step whose agent asked a
//! question resumes as soon as an answer comes; a run that succeeded is followed up (Ask agent). A
//! turn that ends after messages were delivered acknowledges them, and its final message is the
//! agent's reply. What people wrote while no runner was attached is delivered once one attaches.
//!
//! A message sent now goes to the agent at work it reaches while its turn runs
//! ([`RunTask::steer`]); one its turn ended before is read at the turn's end like any other.
//!
//! [`Command::DeliverMessages`](orchester_ipc::Command) only wakes the run: what is queued is read
//! from the database.

use std::path::Path;

use orchester_core::{
  conversation::{self, AgentMessage, Arrival, Delivery, Message, MessageBody},
  id::{ResourceId, RunId, StepExecutionId},
  run::{Invocation, Run, RunStatus, StepExecutor, StepFailure, StepOutcome, StepOutput},
  store::{ConversationStore, MergeStore},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;

use super::{
  Scheduler,
  control::{Ack, Control, answer},
  failed, idle,
  prepare::{self, Prepared},
  run::{ExecutionError, RunTask},
  step::Active,
};

impl Scheduler {
  /// Delivers the messages queued for `run` at its next invocation boundary: a task executing the
  /// run takes them as its agents reach one; an idle run whose agent waits for an answer resumes,
  /// and one that succeeded is followed up. Returns once that is arranged; a paused or stopped run
  /// keeps them queued until it continues.
  pub async fn deliver_messages(&self, run: RunId) -> Result<(), Rejection> {
    loop {
      let one_at_a_time = self.inner.continuing.lock().await;
      if self.inner.board.is_stopping() {
        return Err(Rejection::ShuttingDown);
      }
      if let Some(answer) = self.ask(run, Control::DeliverMessages) {
        drop(one_at_a_time);
        if let Ok(result) = answer.await {
          result?;
          let stored = idle::load(&self.open()?, run)?;
          if stored.status().is_terminal() {
            // The task may be finishing just as the message arrives. Wait for its removal,
            // then deliver through the idle path instead of acknowledging an unread message.
            tokio::task::yield_now().await;
            continue;
          }
          return Ok(());
        }
        continue;
      }
      return self.deliver_idle(run);
    }
  }

  fn deliver_idle(&self, run: RunId) -> Result<(), Rejection> {
    // No task executes the run, or its task ended without reading the request.
    let mut db = self.open()?;
    let Some(prepared) = deliver_idle(&mut db, &self.inner.project_root, run)? else {
      return Ok(());
    };
    self.inner.notifier.send(Event::RunChanged {
      run,
      task: prepared.run.task(),
    });
    // Needs no free agent slot: the run waits for one like any other.
    self.spawn(db, prepared)
  }

  /// Executes again those of `waiting`, runs that only wait for a person, whose agent asked a
  /// question while people's messages are queued for the run: written while no runner was
  /// attached, they would otherwise wait for the next message. The run's task resumes the step the
  /// messages reach, or ends again at once.
  pub(super) fn wake_answered(&self, db: &ProjectDb, waiting: Vec<Run>) -> Result<(), Rejection> {
    for run in waiting {
      if self.is_active(run.id()) || run.awaiting_answer().is_empty() {
        continue;
      }
      let messages = db
        .messages(run.task())
        .map_err(|error| failed("cannot read the task's conversation", &error))?;
      if !messages.iter().any(|message| message.queued_for(run.id()).is_some()) {
        continue;
      }
      tracing::info!(run = %run.id(), "a question was answered while no runner was attached");
      let prepared = Prepared::continuing(db, &self.inner.project_root, run)?;
      self.spawn(self.open()?, prepared)?;
    }
    Ok(())
  }

  /// Follows up the runs that succeeded, and were not merged, while people's messages are queued
  /// for them: written while no runner was attached, they would otherwise wait for the next
  /// message. Each is followed up once, as [`Scheduler::deliver_messages`] does; its messages then
  /// no longer wait. A run that cannot be followed up keeps them queued.
  pub(super) fn follow_up_queued(&self, db: &mut ProjectDb) -> Result<(), Rejection> {
    let runs = db
      .runs_with_queued_messages()
      .map_err(|error| failed("cannot read the queued messages", &error))?;
    for run in runs.into_iter().filter(|&run| !self.is_active(run)) {
      match follow_up_after_attach(db, &self.inner.project_root, run) {
        Ok(Some(prepared)) => {
          tracing::info!(%run, "messages were written to a run ready for review while no runner was attached");
          self.inner.notifier.send(Event::RunChanged {
            run,
            task: prepared.run.task(),
          });
          self.spawn(self.open()?, prepared)?;
        }
        Ok(None) => {}
        Err(rejection) => tracing::warn!(%run, ?rejection, "queued messages cannot be delivered; they stay queued"),
      }
    }
    Ok(())
  }
}

/// What delivering messages does to `run` while no task executes it: an ended or merged run takes
/// none, a succeeded one is followed up and executes again, one whose agent waits for an answer executes
/// again to resume it; anything else keeps them queued.
fn deliver_idle(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Option<Prepared>, Rejection> {
  let stored = idle::load(db, run)?;
  match stored.status() {
    RunStatus::Failed(_) => super::recovery::recover(db, root, stored),
    RunStatus::Cancelled => Err(Rejection::RunEnded),
    // Its work was merged into its base branch: the task is done.
    RunStatus::Succeeded if merged(db, run)? => Err(Rejection::RunEnded),
    RunStatus::Succeeded => follow_up(db, root, stored),
    RunStatus::Running if !stored.awaiting_answer().is_empty() => Prepared::continuing(db, root, stored).map(Some),
    _ => Ok(None),
  }
}

/// What attaching does to `run`, for which people's messages are queued: one that succeeded and was
/// not merged is followed up; anything else keeps them queued.
fn follow_up_after_attach(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Option<Prepared>, Rejection> {
  let stored = idle::load(db, run)?;
  if matches!(stored.status(), RunStatus::Failed(_)) {
    return super::recovery::recover(db, root, stored);
  }
  if stored.status() != &RunStatus::Succeeded || merged(db, run)? {
    return Ok(None);
  }
  follow_up(db, root, stored)
}

fn merged(db: &ProjectDb, run: RunId) -> Result<bool, Rejection> {
  Ok(
    db.run_merge(run)
      .map_err(|error| failed("cannot read the run's merge", &error))?
      .is_some(),
  )
}

/// Follows up `run`, which succeeded, for the oldest message queued for one of its agents; `None`
/// when no queued message reaches one. Its agent continues in the run's worktree, so a run whose
/// worktree was removed is refused and keeps its messages queued.
fn follow_up(db: &mut ProjectDb, root: &Path, mut run: Run) -> Result<Option<Prepared>, Rejection> {
  let messages = db
    .messages(run.task())
    .map_err(|error| failed("cannot read the task's conversation", &error))?;
  let Some(step) = conversation::follow_up_step(&run, &messages) else {
    return Ok(None);
  };
  // Refused before the run changes: a follow-up without its worktree would be recorded as running
  // with no agent at work.
  prepare::existing_worktree(root, run.id())?;
  let events = run.follow_up(&step, Timestamp::now()).map_err(idle::rejection)?;
  idle::store(db, &run, &events)?;
  tracing::info!(run = %run.id(), %step, "follow-up started");
  Prepared::continuing(db, root, run).map(Some)
}

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
    let messages = self.db.messages(self.run.task())?;
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
    let messages = self.db.messages(self.run.task())?;
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
    let mut acknowledged: Vec<Message> = self
      .db
      .messages(self.run.task())?
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
  /// run is to pause.
  pub(super) fn answer_questions(&mut self) -> Result<(), ExecutionError> {
    if self.run.pause_requested() {
      return Ok(());
    }
    for execution in self.run.awaiting_answer() {
      if !self.queued_for(execution)?.is_empty() {
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
    let messages = self.db.messages(self.run.task())?;
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
