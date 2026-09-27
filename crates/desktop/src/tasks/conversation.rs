//! The task conversation as the Conversation tab shows it (02.12 `jgqPe`: the panel `kjsmd`, its
//! messages `J5MIT`): each message with who wrote it and where it stands, whom a new message can
//! go to, which agents take part, and what the run's state means for a message written now. The
//! messages come from the project database: a person's are written here, their delivery and the
//! agents' replies and questions by the runner. Sending blocks on SQLite, so the UI calls it off
//! the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  conversation::{AgentMessage, Delivery, Message, MessageBody, MessageError, Recipient, Sender},
  id::{IdempotencyKey, MessageId, ResourceId, RunId, StepExecutionId, TaskId},
  resource::StepKey,
  run::{PauseReason, RunStatus, StepExecutor, StepStatus, TaskPhase, WaitReason},
  store::ConversationStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::detail::{RunView, TaskDetail};
use crate::messages::{self, Message as UiMessage};

/// Where the task's conversation stands, for the panel's status label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelState {
  /// The task has no run yet.
  NotStarted,
  /// Agents work: messages reach them at their next checkpoint.
  Live,
  /// A step waits for the person: an answer, a plan decision.
  Waiting,
  Paused,
  /// Stopped at a limit.
  Stopped,
  Interrupted,
  /// Ready for review: a message asks the agent again.
  Ready,
  /// Its work was merged into its base branch: no agent reads messages any more.
  Merged,
  /// Failed or cancelled: no agent reads messages any more.
  Ended,
}

impl PanelState {
  pub fn of(phase: TaskPhase) -> Self {
    match phase {
      TaskPhase::Draft | TaskPhase::Queued => Self::NotStarted,
      TaskPhase::Running => Self::Live,
      TaskPhase::Waiting => Self::Waiting,
      TaskPhase::Paused => Self::Paused,
      TaskPhase::StoppedAtLimit => Self::Stopped,
      TaskPhase::Interrupted => Self::Interrupted,
      TaskPhase::Succeeded => Self::Ready,
      TaskPhase::Merged => Self::Merged,
      TaskPhase::Failed | TaskPhase::Cancelled => Self::Ended,
    }
  }

  /// Whether a message written now can reach an agent of the task.
  pub const fn takes_messages(self) -> bool {
    !matches!(self, Self::Merged | Self::Ended)
  }
}

/// Who wrote an entry, as its heading names them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Author {
  /// The person, to Orchestrator.
  You,
  /// The person, to one agent.
  YouTo(Arc<str>),
  Agent(Arc<str>),
}

/// Where an entry stands, as its last line says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryState {
  /// A person's message no agent read yet, and when one will.
  Queued(Queue),
  Delivered {
    agent: Arc<str>,
  },
  Acknowledged {
    agent: Arc<str>,
  },
  /// An agent's reply at the end of a turn of `step`.
  Reply {
    step: Arc<str>,
  },
  /// An agent's question; `open` while its step waits for the answer.
  Question {
    open: bool,
  },
}

/// When a queued message reaches an agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queue {
  /// At the next checkpoint of an agent at work, or the next one to start.
  NextCheckpoint,
  /// Once the task's run starts.
  RunStarts,
  /// Once the paused, stopped or interrupted run continues.
  RunContinues,
  /// Never: the run it was written for has ended.
  RunEnded,
}

/// One message of the conversation as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
  pub id: MessageId,
  pub author: Author,
  pub at: Timestamp,
  pub body: Arc<str>,
  pub state: EntryState,
}

/// Whom a new message can go to: Orchestrator, or an agent of the run's pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipientChoice {
  pub recipient: Recipient,
  /// The agent's role; `None` for Orchestrator.
  pub agent: Option<Arc<str>>,
}

/// The agents of the run, by what they do now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Participants {
  pub working: Vec<Arc<str>>,
  pub asking: Vec<Arc<str>>,
  pub available: Vec<Arc<str>>,
}

/// The task's conversation, ready to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversation {
  pub state: PanelState,
  pub entries: Vec<Entry>,
  pub recipients: Vec<RecipientChoice>,
  pub participants: Participants,
  /// The agent a message to Orchestrator reaches next when the run is ready for review or waits
  /// for an answer; `None` otherwise.
  pub next_agent: Option<Arc<str>>,
}

impl lurq::app::component::DevtoolsInspectable for Conversation {}

impl Conversation {
  /// The conversation of `detail`'s task; `lost`: the run is recorded as running without a runner.
  pub fn of(detail: &TaskDetail, lost: bool) -> Self {
    let phase = detail.phase();
    let state = PanelState::of(if lost { phase.without_runner() } else { phase });
    let run = detail.run.as_ref();
    let entries = detail
      .messages
      .iter()
      .map(|message| entry(message, run, &detail.messages, state))
      .collect();
    Self {
      state,
      entries,
      recipients: run.map(recipients).unwrap_or_else(|| vec![orchestrator()]),
      participants: run.map(participants).unwrap_or_default(),
      next_agent: run.and_then(|run| next_agent(run, state)),
    }
  }

  /// The open question, if the run waits for an answer to one.
  pub fn open_question(&self) -> Option<&Entry> {
    self
      .entries
      .iter()
      .find(|entry| entry.state == EntryState::Question { open: true })
  }
}

fn orchestrator() -> RecipientChoice {
  RecipientChoice {
    recipient: Recipient::Orchestrator,
    agent: None,
  }
}

/// Orchestrator, then each agent profile of the run's pipeline once, in pipeline order.
fn recipients(run: &RunView) -> Vec<RecipientChoice> {
  let mut choices = vec![orchestrator()];
  for step in &run.steps {
    let (Some(profile), Some(agent)) = (profile_of(run, &step.key), step.agent.clone()) else {
      continue;
    };
    if step.tool_step
      || choices
        .iter()
        .any(|choice| choice.recipient == Recipient::Agent(profile))
    {
      continue;
    }
    choices.push(RecipientChoice {
      recipient: Recipient::Agent(profile),
      agent: Some(agent),
    });
  }
  choices
}

/// The agent profile step `step` of the run runs on; `None` for a tool step.
fn profile_of(run: &RunView, step: &StepKey) -> Option<ResourceId> {
  match run.run.snapshot().step(step).map(|pinned| pinned.executor()) {
    Some(StepExecutor::Agent { profile, .. }) => Some(profile.resource),
    _ => None,
  }
}

/// The run's agents, each once: working when a step of theirs runs, asking when one waits for an
/// answer, available otherwise.
fn participants(run: &RunView) -> Participants {
  let mut participants = Participants::default();
  for step in run.steps.iter().filter(|step| !step.tool_step) {
    let Some(agent) = step.agent.clone() else {
      continue;
    };
    let group = match step.status {
      Some(StepStatus::Running) if run.run.status() == &RunStatus::Running => &mut participants.working,
      Some(StepStatus::Waiting(WaitReason::Message)) => &mut participants.asking,
      _ => &mut participants.available,
    };
    if !group.contains(&agent) {
      group.push(agent);
    }
  }
  let busy: Vec<Arc<str>> = participants
    .working
    .iter()
    .chain(&participants.asking)
    .cloned()
    .collect();
  participants.available.retain(|agent| !busy.contains(agent));
  participants
}

/// The agent a message reaches next: the one that asked, or the one a follow-up would ask.
fn next_agent(run: &RunView, state: PanelState) -> Option<Arc<str>> {
  let execution = match state {
    PanelState::Waiting => *run.run.awaiting_answer().first()?,
    PanelState::Ready => {
      let step = run.run.follow_up_step(None)?;
      run.run.latest_execution(&step)?.id()
    }
    _ => return None,
  };
  run.step_of(execution).and_then(|step| step.agent.clone())
}

fn entry(message: &Message, run: Option<&RunView>, messages: &[Message], state: PanelState) -> Entry {
  let agent_of = |execution: StepExecutionId| -> Arc<str> {
    run
      .and_then(|run| run.step_of(execution))
      .map(|step| step.agent.clone().unwrap_or_else(|| step.name.clone()))
      .unwrap_or_default()
  };
  let (author, state) = match *message.sender() {
    Sender::Person { recipient, delivery } => {
      let author = match recipient {
        Recipient::Orchestrator => Author::You,
        Recipient::Agent(profile) => Author::YouTo(role_of(run, profile)),
      };
      let state = match delivery {
        Delivery::Queued => EntryState::Queued(queue(message, run, state)),
        Delivery::Delivered { execution, .. } => EntryState::Delivered {
          agent: agent_of(execution),
        },
        Delivery::Acknowledged { execution, .. } => EntryState::Acknowledged {
          agent: agent_of(execution),
        },
      };
      (author, state)
    }
    Sender::Agent { execution, kind, .. } => {
      let state = match kind {
        AgentMessage::Reply => EntryState::Reply {
          step: run
            .and_then(|run| run.step_of(execution))
            .map(|step| step.name.clone())
            .unwrap_or_default(),
        },
        AgentMessage::Question => EntryState::Question {
          open: run.is_some_and(|run| message.awaits_answer(&run.run, messages)),
        },
      };
      (Author::Agent(agent_of(execution)), state)
    }
  };
  Entry {
    id: message.id(),
    author,
    at: message.created_at(),
    body: message.body().as_str().into(),
    state,
  }
}

/// The role of agent profile `profile` among the run's steps; empty once no step names it.
fn role_of(run: Option<&RunView>, profile: ResourceId) -> Arc<str> {
  run
    .and_then(|run| {
      run
        .steps
        .iter()
        .find(|step| profile_of(run, &step.key) == Some(profile))
        .and_then(|step| step.agent.clone())
    })
    .unwrap_or_default()
}

/// When a queued message reaches an agent, given the run it was written for.
fn queue(message: &Message, run: Option<&RunView>, state: PanelState) -> Queue {
  let Some(run) = run else {
    return Queue::RunStarts;
  };
  if message.run().is_some_and(|own| own != run.id()) {
    return Queue::RunEnded;
  }
  match run.run.status() {
    RunStatus::Pending => Queue::RunStarts,
    RunStatus::Paused(PauseReason::Requested | PauseReason::LimitReached(_)) | RunStatus::Interrupted(_) => {
      Queue::RunContinues
    }
    RunStatus::Failed(_) | RunStatus::Cancelled => Queue::RunEnded,
    // A run recorded as running without a runner is interrupted until one is back.
    RunStatus::Running if state == PanelState::Interrupted => Queue::RunContinues,
    RunStatus::Running | RunStatus::Succeeded => Queue::NextCheckpoint,
  }
}

/// Why a message was not sent.
#[derive(Debug)]
pub enum SendError {
  Blank,
  Store(StoreError),
}

impl SendError {
  pub fn message(&self) -> UiMessage {
    match self {
      Self::Blank => UiMessage::new("conversation.errors.blank"),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

/// Stores a person's message `text` to `recipient` for `task`'s latest run (`run`, if it has one),
/// once per `key`: a retry after an unclear failure returns the message stored the first time.
pub fn send_in(
  repo_root: &Path,
  key: IdempotencyKey,
  (task, run): (TaskId, Option<RunId>),
  recipient: Recipient,
  text: &str,
) -> Result<Message, SendError> {
  // A new message's text is only checked for being blank.
  let body = MessageBody::new(text).map_err(|_: MessageError| SendError::Blank)?;
  let message = Message::from_person(task, run, recipient, body, Timestamp::now());
  let mut db = ProjectDb::open(repo_root).map_err(SendError::Store)?;
  db.send_message(key, &message).map_err(SendError::Store)
}
