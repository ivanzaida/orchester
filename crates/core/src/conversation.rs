//! The task conversation (`docs/design/orchester-workspace.md`, "Task conversation"): what a person
//! writes to the agents of a task and what the agents write back. A person's message goes to
//! Orchestrator, which routes it to the agent of the task's run that works next, or to the agent of
//! one profile. It is queued until an agent invocation of the run starts with it in its prompt
//! (delivered), and acknowledged once that agent ended a turn after reading it: it answered or
//! acted on it. Acknowledgement does not prove that a requested change was applied.
//!
//! An agent writes back in two ways: the reply that ends the turn a message was delivered in, and a
//! question — a step result that asks the person before the step can go on
//! ([`crate::run::StepOutcome::Question`]); its step waits for a message
//! ([`crate::run::WaitReason::Message`]) and the answer resumes its session.
//!
//! People's messages are written by the desktop; delivery states and agents' messages by the
//! runner (`docs/architecture/storage.md`, "Write ownership").

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  id::{InvocationId, MessageId, ResourceId, RunId, StepExecutionId, TaskId},
  resource::StepKey,
  run::{EventSequence, Run, StepStatus, WaitReason},
  time::Timestamp,
};

/// The text of a message: never blank, without the whitespace around it.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MessageBody(String);

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MessageError {
  #[error("a message says something")]
  Blank,
  #[error("only a person's message is delivered")]
  NotFromAPerson,
  #[error("the message was delivered already")]
  AlreadyDelivered,
  #[error("the message has not been delivered")]
  NotDelivered,
  #[error("the message was written for another run")]
  OtherRun,
  #[error("an agent's message and a delivered one belong to a run")]
  WithoutRun,
}

impl MessageBody {
  pub fn new(text: &str) -> Result<Self, MessageError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(MessageError::Blank);
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for MessageBody {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Debug::fmt(&self.0, f)
  }
}

impl TryFrom<String> for MessageBody {
  type Error = MessageError;

  fn try_from(text: String) -> Result<Self, Self::Error> {
    Self::new(&text)
  }
}

impl From<MessageBody> for String {
  fn from(body: MessageBody) -> Self {
    body.0
  }
}

/// Whom a person's message is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "profile")]
pub enum Recipient {
  /// Orchestrator routes it to the agent of the task's run whose invocation starts next.
  #[default]
  Orchestrator,
  /// The agent of this profile, at its next invocation in the run.
  Agent(ResourceId),
}

/// Where a person's message stands: queued → delivered → acknowledged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
  /// Waiting for an invocation to start with it.
  Queued,
  /// Invocation `invocation` of `execution` started with it in its prompt at `at`.
  Delivered {
    execution: StepExecutionId,
    invocation: InvocationId,
    at: Timestamp,
  },
  /// `execution`'s agent ended a turn after reading it, at `at`.
  Acknowledged {
    execution: StepExecutionId,
    invocation: InvocationId,
    delivered_at: Timestamp,
    at: Timestamp,
  },
}

impl Delivery {
  /// The execution it was delivered to, once it was.
  pub const fn execution(self) -> Option<StepExecutionId> {
    match self {
      Self::Queued => None,
      Self::Delivered { execution, .. } | Self::Acknowledged { execution, .. } => Some(execution),
    }
  }
}

/// What an agent's message is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessage {
  /// What the agent answered at the end of a turn it read a person's messages in.
  Reply,
  /// The agent cannot go on without the person's answer; its step waits for a message.
  Question,
}

/// Who wrote a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sender {
  Person {
    recipient: Recipient,
    delivery: Delivery,
  },
  /// The agent of `execution`, at the end of invocation `invocation`.
  Agent {
    execution: StepExecutionId,
    invocation: InvocationId,
    kind: AgentMessage,
  },
}

/// One message of a task's conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
  id: MessageId,
  task: TaskId,
  /// The run it was written for or in; `None` for a person's message written before the task had a
  /// run, which the first invocation of its next run takes.
  run: Option<RunId>,
  sender: Sender,
  body: MessageBody,
  created_at: Timestamp,
  recovery_after: Option<EventSequence>,
}

impl Message {
  /// A person's new message to `recipient`, queued for `run` (the task's latest run, if it has one).
  pub fn from_person(
    task: TaskId,
    run: Option<RunId>,
    recipient: Recipient,
    body: MessageBody,
    now: Timestamp,
  ) -> Self {
    Self {
      id: MessageId::generate(),
      task,
      run,
      sender: Sender::Person {
        recipient,
        delivery: Delivery::Queued,
      },
      body,
      created_at: now,
      recovery_after: None,
    }
  }

  /// What the agent of `execution` in `run` wrote at the end of `invocation`.
  pub fn from_agent(
    task: TaskId,
    run: RunId,
    (execution, invocation): (StepExecutionId, InvocationId),
    kind: AgentMessage,
    body: MessageBody,
    now: Timestamp,
  ) -> Self {
    Self {
      id: MessageId::generate(),
      task,
      run: Some(run),
      sender: Sender::Agent {
        execution,
        invocation,
        kind,
      },
      body,
      created_at: now,
      recovery_after: None,
    }
  }

  /// Rebuilds a stored message; an agent's message and a delivered one always name their run.
  pub fn restore(
    id: MessageId,
    task: TaskId,
    run: Option<RunId>,
    sender: Sender,
    body: MessageBody,
    created_at: Timestamp,
  ) -> Result<Self, MessageError> {
    let in_run = match sender {
      Sender::Agent { .. } => true,
      Sender::Person { delivery, .. } => delivery.execution().is_some(),
    };
    if in_run && run.is_none() {
      return Err(MessageError::WithoutRun);
    }
    Ok(Self {
      id,
      task,
      run,
      sender,
      body,
      created_at,
      recovery_after: None,
    })
  }

  pub const fn id(&self) -> MessageId {
    self.id
  }

  pub const fn task(&self) -> TaskId {
    self.task
  }

  pub const fn run(&self) -> Option<RunId> {
    self.run
  }

  pub const fn sender(&self) -> &Sender {
    &self.sender
  }

  pub const fn body(&self) -> &MessageBody {
    &self.body
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  /// The failure the person explicitly chose to continue after. A message queued before a
  /// failure cannot retry it; another failure requires another explicit choice.
  pub const fn recovery_after(&self) -> Option<EventSequence> {
    self.recovery_after
  }

  pub fn recovering_after(mut self, failure: EventSequence) -> Result<Self, MessageError> {
    if !matches!(self.sender, Sender::Person { .. }) {
      return Err(MessageError::NotFromAPerson);
    }
    if self.run.is_none() {
      return Err(MessageError::WithoutRun);
    }
    self.recovery_after = Some(failure);
    Ok(self)
  }

  /// A person's message and where it stands; `None` for an agent's.
  pub const fn delivery(&self) -> Option<Delivery> {
    match self.sender {
      Sender::Person { delivery, .. } => Some(delivery),
      Sender::Agent { .. } => None,
    }
  }

  /// The recipient of a person's message still queued for `run`: written for that run, or before
  /// the task had one. `None` for any other message.
  pub fn queued_for(&self, run: RunId) -> Option<Recipient> {
    match self.sender {
      Sender::Person {
        recipient,
        delivery: Delivery::Queued,
      } if self.run.is_none_or(|own| own == run) => Some(recipient),
      _ => None,
    }
  }

  /// Whether this is a person's queued message the agent of `profile` takes at its next invocation
  /// in `run`: queued for that run, to Orchestrator or to `profile`.
  pub fn reaches(&self, run: RunId, profile: ResourceId) -> bool {
    match self.queued_for(run) {
      Some(Recipient::Orchestrator) => true,
      Some(Recipient::Agent(agent)) => agent == profile,
      None => false,
    }
  }

  /// The message went to `execution`'s agent of `run` with `invocation`, started at `at`.
  pub fn deliver(
    &mut self,
    run: RunId,
    (execution, invocation): (StepExecutionId, InvocationId),
    at: Timestamp,
  ) -> Result<(), MessageError> {
    let Sender::Person { delivery, .. } = &mut self.sender else {
      return Err(MessageError::NotFromAPerson);
    };
    if *delivery != Delivery::Queued {
      return Err(MessageError::AlreadyDelivered);
    }
    if self.run.is_some_and(|own| own != run) {
      return Err(MessageError::OtherRun);
    }
    *delivery = Delivery::Delivered {
      execution,
      invocation,
      at,
    };
    self.run = Some(run);
    Ok(())
  }

  /// The agent it was delivered to ended a turn after reading it, at `at`.
  pub fn acknowledge(&mut self, at: Timestamp) -> Result<(), MessageError> {
    let Sender::Person { delivery, .. } = &mut self.sender else {
      return Err(MessageError::NotFromAPerson);
    };
    let Delivery::Delivered {
      execution,
      invocation,
      at: delivered_at,
    } = *delivery
    else {
      return Err(MessageError::NotDelivered);
    };
    *delivery = Delivery::Acknowledged {
      execution,
      invocation,
      delivered_at,
      at,
    };
    Ok(())
  }

  /// Whether this is the question `run` waits for an answer to: the latest question of an execution
  /// whose step waits for a message.
  pub fn awaits_answer(&self, run: &Run, messages: &[Self]) -> bool {
    let Sender::Agent {
      execution,
      kind: AgentMessage::Question,
      ..
    } = self.sender
    else {
      return false;
    };
    let waiting = run
      .execution(execution)
      .is_some_and(|step| step.status() == StepStatus::Waiting(WaitReason::Message));
    waiting && latest_question(messages, execution).is_some_and(|latest| latest.id == self.id)
  }
}

/// The step a follow-up of `run`, which succeeded, asks for `messages`: the step of the agent the
/// oldest message still queued for it goes to ([`Run::follow_up_step`]). `None` when no queued
/// message reaches an agent of the run.
pub fn follow_up_step(run: &Run, messages: &[Message]) -> Option<StepKey> {
  messages
    .iter()
    .filter_map(|message| message.queued_for(run.id()))
    .find_map(|recipient| match recipient {
      Recipient::Orchestrator => run.follow_up_step(None),
      Recipient::Agent(profile) => run.follow_up_step(Some(profile)),
    })
}

/// The latest question `execution`'s agent asked, among `messages`.
pub fn latest_question(messages: &[Message], execution: StepExecutionId) -> Option<&Message> {
  messages.iter().rev().find(|message| {
    matches!(
      message.sender,
      Sender::Agent { execution: asked, kind: AgentMessage::Question, .. } if asked == execution
    )
  })
}
