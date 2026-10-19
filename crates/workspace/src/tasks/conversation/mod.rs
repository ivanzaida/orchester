//! The task conversation as the Conversation tab shows it (`docs/design/orchester-workspace.md`,
//! "Conversation (merged with Activity)"): each message with who wrote it, the step it belongs to and
//! where it stands (sending now, queued, delivered mid-turn or at the end of a turn, not delivered),
//! whom a new message can go to, which agents take part and work now, and what the run's state means
//! for a message written now. The messages come from the project database: a person's are written
//! here, their delivery and the agents' replies and questions by the runner. Sending blocks on
//! SQLite, so the UI calls it off the UI thread.

use std::sync::Arc;

use orchester_core::{
  conversation::{AgentMessage, Arrival, Delivery, Message, Recipient, Sender, Timing, Undelivered},
  id::{MessageId, ResourceId, StepExecutionId},
  integration::Leftovers,
  resource::StepKey,
  run::{EventSequence, PauseReason, RunStatus, StepExecutor, StepStatus, TaskPhase, can_run_again_after},
  time::Timestamp,
};

pub use self::send::{SendError, send_after_turn_as, send_after_turn_in, send_in};
use super::detail::{RunView, TaskDetail};

pub mod composer;
pub mod follow;
pub mod outgoing;
mod send;

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
  /// The run waits for the person's decision after its retries ran out or a step broke again; a
  /// new message continues it from an agent's work, as after a failure.
  Deciding,
  Interrupted,
  /// Ready for review: a message asks the agent again.
  Ready,
  /// A new message continues the failed run from an existing agent's work.
  Failed,
  /// A failed run whose worktree was removed cannot resume its agent.
  FailedWorktreeRemoved,
  /// The failed pipeline has no executed agent to receive a message.
  FailedNoAgent,
  /// Ready for review, but its worktree was removed: its agent cannot continue, so no agent reads
  /// messages any more.
  WorktreeRemoved,
  /// Its work was merged into its base branch: no agent reads messages any more.
  Merged,
  /// Cancelled: no agent reads messages any more.
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
      TaskPhase::Stuck => Self::Deciding,
      TaskPhase::Interrupted => Self::Interrupted,
      TaskPhase::Succeeded => Self::Ready,
      TaskPhase::Merged => Self::Merged,
      TaskPhase::Failed => Self::Failed,
      TaskPhase::Cancelled => Self::Ended,
    }
  }

  /// Whether a message written now can reach an agent of the task's latest run.
  pub const fn takes_messages(self) -> bool {
    !matches!(
      self,
      Self::Merged | Self::WorktreeRemoved | Self::FailedWorktreeRemoved | Self::FailedNoAgent | Self::Ended
    )
  }

  /// Why no agent of the run reads a message written now, as the closed composer says it.
  pub const fn closed_reason(self) -> &'static str {
    match self {
      Self::Merged => "conversation.explanation.merged",
      Self::WorktreeRemoved | Self::FailedWorktreeRemoved => "conversation.explanation.worktreeRemoved",
      Self::FailedNoAgent => "conversation.explanation.failedNoAgent",
      _ => "conversation.explanation.ended",
    }
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

/// Where an entry stands, as its delivery line says. A delivered message keeps its line once its
/// agent read it: the agent's reply shows in the timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryState {
  /// A person's message no agent read yet, and when one will.
  Queued(Queue),
  /// It reached `agent` at `at`, as `arrival` says; `sent_now`: the person sent it now.
  Delivered {
    agent: Arc<str>,
    at: Timestamp,
    arrival: Arrival,
    sent_now: bool,
  },
  /// Sent now, `agent`'s CLI did not take it; it is kept and goes no further on its own.
  NotDelivered { agent: Arc<str>, reason: Undelivered },
  /// An agent's reply at the end of a turn of `step`.
  Reply { step: Arc<str> },
  /// An agent's question; `open` while its step waits for the answer.
  Question { open: bool },
}

/// When a queued message reaches an agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Queue {
  /// Sent now to `0`, which works: it gets it at its next tool call.
  SendingNow(Arc<str>),
  /// Sent after the turn to `0`, which works: it reads it when its turn ends.
  AfterTurn(Arc<str>),
  /// At the next checkpoint of an agent at work, or the next one to start.
  NextCheckpoint,
  /// Once the task's run starts.
  RunStarts,
  /// Before the next action after a person resumes a paused run, by `0` when it is known.
  RunResumes(Option<Arc<str>>),
  /// Once the stopped, interrupted, failed or waiting run continues, by `0` when it is known.
  RunContinues(Option<Arc<str>>),
  /// At the next turn of `0`, a direct recipient that does not work now.
  AgentNextTurn(Arc<str>),
  /// Never: the run it was written for has ended.
  RunEnded,
  /// Once the task's next run starts: it was written for it, after the latest run ended.
  NextRun,
  /// Never: the run's worktree was removed, so its agent cannot continue.
  WorktreeRemoved,
}

/// One message of the conversation as the timeline shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
  pub id: MessageId,
  pub author: Author,
  pub at: Timestamp,
  pub body: Arc<str>,
  pub state: EntryState,
  /// The step execution it belongs to: the one it reached or did not, or whose agent wrote it;
  /// `None` while it waits for one.
  pub execution: Option<StepExecutionId>,
  /// A person's message sent now.
  pub sent_now: bool,
  /// A message on the person's side that the project's Orchestrator chat wrote and sent: its heading names the
  /// Orchestrator.
  pub by_orchestrator: bool,
  /// Automation wrote this answer or continuation on the person's side.
  pub by_automation: bool,
}

/// An agent at work on the run: its profile, its role, the execution it works on and whether its
/// connection takes a message while its turn runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingAgent {
  pub profile: ResourceId,
  pub agent: Arc<str>,
  pub execution: StepExecutionId,
  pub mid_turn: bool,
}

/// Whom a new message can go to: Orchestrator, or an agent of the run's pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipientChoice {
  pub recipient: Recipient,
  /// The agent's role; `None` for Orchestrator.
  pub agent: Option<Arc<str>>,
}

/// The task's conversation, ready to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversation {
  pub state: PanelState,
  pub recovery_after: Option<EventSequence>,
  pub entries: Vec<Entry>,
  pub recipients: Vec<RecipientChoice>,
  /// The agent a message to Orchestrator reaches next when the run is ready for review or waits
  /// for an answer; `None` otherwise.
  pub next_agent: Option<Arc<str>>,
  /// The agents at work now, in pipeline order: a message for one of them can be sent now.
  pub working: Vec<WorkingAgent>,
  /// The agent of the run's first step, which a message reaches when the run starts.
  pub first_agent: Option<Arc<str>>,
}

impl Conversation {
  /// The conversation of `detail`'s task; `lost`: the run is recorded as running without a runner;
  /// `leftovers`: what the run left once it ended, once read.
  pub fn of(detail: &TaskDetail, lost: bool, leftovers: Option<&Leftovers>) -> Self {
    let phase = detail.phase();
    let state = match PanelState::of(if lost { phase.without_runner() } else { phase }) {
      PanelState::Ready if leftovers.is_some_and(|leftovers| !leftovers.keeps_run_worktree()) => {
        PanelState::WorktreeRemoved
      }
      PanelState::Failed if leftovers.is_some_and(|leftovers| !leftovers.keeps_run_worktree()) => {
        PanelState::FailedWorktreeRemoved
      }
      PanelState::Failed
        if detail
          .run
          .as_ref()
          .is_none_or(|run| run.run.recovery_step(None).is_none()) =>
      {
        PanelState::FailedNoAgent
      }
      state => state,
    };
    let run = detail.run.as_ref();
    let working = run
      .filter(|_| state == PanelState::Live)
      .map(working_agents)
      .unwrap_or_default();
    let entries = detail
      .messages
      .iter()
      .map(|message| entry(message, (run, &working), &detail.messages, state))
      .collect();
    Self {
      state,
      recovery_after: run
        .filter(|run| {
          state == PanelState::Failed || (state == PanelState::Deciding && run.run.recovery_step(None).is_some())
        })
        .map(|run| run.run.last_sequence()),
      entries,
      recipients: run.map(recipients).unwrap_or_else(|| vec![orchestrator()]),
      next_agent: run.and_then(|run| next_agent(run, state)),
      working,
      first_agent: run
        .and_then(|run| run.steps.iter().find_map(|step| step.agent.clone()))
        .or_else(|| detail.first_agent.clone()),
    }
  }

  /// The agent at work that a message to `recipient` reaches now, if one does.
  pub fn working_for(&self, recipient: Recipient) -> Option<&WorkingAgent> {
    match recipient {
      Recipient::Orchestrator => self.working.first(),
      Recipient::Agent(profile) => self.working.iter().find(|working| working.profile == profile),
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

/// The agents whose invocation runs now, in pipeline order, each once.
fn working_agents(run: &RunView) -> Vec<WorkingAgent> {
  let mut working: Vec<WorkingAgent> = Vec::new();
  for step in run.working_steps().filter(|step| !step.tool_step) {
    let (Some(profile), Some(agent), Some(execution)) =
      (profile_of(run, &step.key), step.agent.clone(), step.execution)
    else {
      continue;
    };
    let at_work = run
      .invocations_of(execution)
      .any(|invocation| invocation.model().is_some() && invocation.ended_at.is_none());
    if at_work && !working.iter().any(|known| known.profile == profile) {
      working.push(WorkingAgent {
        profile,
        agent,
        execution,
        mid_turn: step.provider.is_some_and(|provider| provider.takes_messages_mid_turn()),
      });
    }
  }
  working
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
    let continued = matches!(run.run.status(), RunStatus::Failed(_) | RunStatus::WaitingForPerson(_));
    if step.tool_step
      || (continued && run.run.recovery_step(Some(profile)).is_none())
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

/// The agent a message reaches next: the one that asked, the one a follow-up would ask, the one a
/// continuation resumes, or the one that reads it first once a suspended run goes on.
fn next_agent(run: &RunView, state: PanelState) -> Option<Arc<str>> {
  let execution = match state {
    PanelState::Waiting => *run.run.awaiting_answer().first()?,
    PanelState::Ready => {
      let step = run.run.follow_up_step(None)?;
      run.run.latest_execution(&step)?.id()
    }
    PanelState::Failed | PanelState::Deciding => {
      let step = run.run.recovery_step(None)?;
      run.run.latest_execution(&step)?.id()
    }
    PanelState::Paused | PanelState::Stopped | PanelState::Interrupted => return next_reader(run),
    _ => return None,
  };
  run.step_of(execution).and_then(|step| step.agent.clone())
}

/// The agent that takes a message to Orchestrator first once the suspended `run` goes on: the one
/// of the step it stopped in, else of the first agent step still to start.
fn next_reader(run: &RunView) -> Option<Arc<str>> {
  run
    .stopped_step()
    .or_else(|| run.working_steps().next())
    .filter(|step| !step.tool_step)
    .or_else(|| {
      run
        .steps
        .iter()
        .filter(|step| !step.tool_step)
        .find(|step| matches!(step.status, None | Some(StepStatus::Pending)))
    })
    .and_then(|step| step.agent.clone())
}

fn entry(
  message: &Message,
  (run, working): (Option<&RunView>, &[WorkingAgent]),
  messages: &[Message],
  state: PanelState,
) -> Entry {
  let agent_of = |execution: StepExecutionId| -> Arc<str> {
    run
      .and_then(|run| run.step_of(execution))
      .map(|step| step.agent.clone().unwrap_or_else(|| step.name.clone()))
      .unwrap_or_default()
  };
  let sent_now = message.timing() == Timing::Now;
  let (author, state, execution) = match *message.sender() {
    Sender::Person { recipient, delivery } => {
      let author = match recipient {
        Recipient::Orchestrator => Author::You,
        Recipient::Agent(profile) => Author::YouTo(role_of(run, profile)),
      };
      let (state, execution) = match delivery {
        Delivery::Queued => (EntryState::Queued(queue(message, (run, working), state)), None),
        Delivery::Delivered {
          execution, at, arrival, ..
        }
        | Delivery::Acknowledged {
          execution,
          delivered_at: at,
          arrival,
          ..
        } => (
          EntryState::Delivered {
            agent: agent_of(execution),
            at,
            arrival,
            sent_now,
          },
          Some(execution),
        ),
        Delivery::NotDelivered { execution, reason, .. } => (
          EntryState::NotDelivered {
            agent: agent_of(execution),
            reason,
          },
          Some(execution),
        ),
      };
      (author, state, execution)
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
      (Author::Agent(agent_of(execution)), state, Some(execution))
    }
  };
  Entry {
    id: message.id(),
    author,
    at: message.created_at(),
    body: message.body().as_str().into(),
    state,
    execution,
    sent_now,
    by_orchestrator: message.by_orchestrator(),
    by_automation: message.answerer() == Some(orchester_core::run::Answerer::Automation),
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

/// When a queued message reaches an agent, given the run it was written for and the agents at work.
fn queue(message: &Message, (run, working): (Option<&RunView>, &[WorkingAgent]), state: PanelState) -> Queue {
  let Some(run) = run else {
    return Queue::RunStarts;
  };
  if message.run().is_some_and(|own| own != run.id()) {
    return Queue::RunEnded;
  }
  let recipient = message.queued_for(run.id()).unwrap_or_default();
  let at_work = match recipient {
    Recipient::Orchestrator => working.first(),
    Recipient::Agent(profile) => working.iter().find(|working| working.profile == profile),
  };
  if let Some(working) = at_work {
    return match message.timing() {
      Timing::Now => Queue::SendingNow(working.agent.clone()),
      Timing::AfterTurn => Queue::AfterTurn(working.agent.clone()),
    };
  }
  // Written to run the task again after a run that never continues: the next run's first agent
  // reads it.
  if message.run().is_none()
    && state != PanelState::Failed
    && can_run_again_after(&run.run, run.worktree_removed.is_some())
  {
    return Queue::NextRun;
  }
  let later_agent = direct_recipient_waits_for_later_turn(message, run, state);
  let next_turn = || match recipient {
    Recipient::Agent(profile) => Queue::AgentNextTurn(role_of(Some(run), profile)),
    Recipient::Orchestrator => Queue::NextCheckpoint,
  };
  // Who reads it once the run goes on: its own recipient, or the agent that reads first.
  let reader = || match recipient {
    Recipient::Agent(profile) => Some(role_of(Some(run), profile)).filter(|role| !role.is_empty()),
    Recipient::Orchestrator => next_agent(run, state),
  };
  match run.run.status() {
    RunStatus::Pending => Queue::RunStarts,
    RunStatus::Paused(PauseReason::Requested) if later_agent => next_turn(),
    RunStatus::Paused(PauseReason::Requested) => Queue::RunResumes(reader()),
    RunStatus::Paused(PauseReason::LimitReached(_)) | RunStatus::Interrupted(_) | RunStatus::WaitingForPerson(_)
      if later_agent =>
    {
      next_turn()
    }
    RunStatus::Paused(PauseReason::LimitReached(_)) | RunStatus::Interrupted(_) | RunStatus::WaitingForPerson(_) => {
      Queue::RunContinues(reader())
    }
    RunStatus::Failed(_) if state == PanelState::FailedWorktreeRemoved => Queue::WorktreeRemoved,
    RunStatus::Failed(_) if state == PanelState::Failed => Queue::RunContinues(reader()),
    RunStatus::Failed(_) | RunStatus::Cancelled => Queue::RunEnded,
    // A run recorded as running without a runner is interrupted until one is back.
    RunStatus::Running if state == PanelState::Interrupted && later_agent => next_turn(),
    RunStatus::Running if state == PanelState::Interrupted => Queue::RunContinues(reader()),
    RunStatus::Succeeded if state == PanelState::WorktreeRemoved => Queue::WorktreeRemoved,
    RunStatus::Running | RunStatus::Succeeded if later_agent => next_turn(),
    RunStatus::Running | RunStatus::Succeeded => Queue::NextCheckpoint,
  }
}

fn direct_recipient_waits_for_later_turn(message: &Message, run: &RunView, state: PanelState) -> bool {
  let Some(Recipient::Agent(profile)) = message.queued_for(run.id()) else {
    return false;
  };
  let step = match state {
    PanelState::Stopped => run.stopped_step().or_else(|| run.current_step()),
    _ => run.current_step(),
  };
  step.and_then(|step| profile_of(run, &step.key)) != Some(profile)
}
