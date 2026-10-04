//! What the conversation's composer offers in the run's state (`docs/design/orchester-workspace.md`,
//! "Conversation (merged with Activity)", Composer): while an agent works and the message is for
//! it, *Send now* and *Send after this turn*; a question answered, a plan's changes requested, a
//! follow-up asked, a failed run continued with instructions, a message queued for a run that has
//! not started or does not work now; or nothing, once no agent reads messages.

use std::sync::Arc;

use orchester_core::{
  conversation::Recipient,
  id::{MessageId, ResourceId},
  run::PlanVersion,
  time::Timestamp,
};

use super::{Author, Conversation, PanelState, WorkingAgent};
use crate::tasks::{detail::RunView, plan::PlanReview};

/// What the composer's primary action does now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerMode {
  /// `agent` works and the message is for it: *Send now* (when its connection takes a message
  /// mid-turn) and *Send after this turn*.
  Steer(WorkingAgent),
  /// The message queues: a tool step runs, the recipient does not work now, or the run is paused,
  /// interrupted, stopped or waits for the person's decision.
  Queue(Queued),
  /// `agent` asked a question at `asked_at`: the reply answers it and resumes its session.
  Answer {
    agent: Arc<str>,
    profile: Option<ResourceId>,
    /// The question's message.
    question: MessageId,
    asked_at: Timestamp,
  },
  /// Plan `version` of `agent` waits for a decision: the message is the changes it needs.
  PlanChanges { agent: Arc<str>, version: PlanVersion },
  /// The run is ready for review: the message follows it up.
  FollowUp,
  /// The run failed: the message continues it with instructions.
  Continue,
  /// The run has not started: the message waits for its first step.
  NotStarted,
  /// No agent reads messages any more.
  Closed,
}

/// Why a message written now queues.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Queued {
  /// A tool step runs, not an agent.
  ToolStep,
  /// The agent the message is for does not work now: it reads the message at its next turn. `0`
  /// names it while the run's pipeline still offers it as a recipient.
  NextTurn(Option<Arc<str>>),
  /// The run is paused, stopped at a limit or interrupted.
  Suspended,
  /// The run waits for the person's decision on its Summary, after a step's retries ran out or it
  /// broke again: the message waits until the run continues.
  Decision,
  /// Agents work, but none is at a turn yet.
  Checkpoint,
}

impl ComposerMode {
  /// The mode for a message to `recipient` in `conversation` of `run`.
  pub fn of(conversation: &Conversation, run: Option<&RunView>, recipient: Recipient) -> Self {
    match conversation.state {
      PanelState::NotStarted => Self::NotStarted,
      PanelState::Live => live(conversation, run, recipient),
      PanelState::Waiting => waiting(conversation, run),
      PanelState::Paused | PanelState::Stopped | PanelState::Interrupted => Self::Queue(Queued::Suspended),
      PanelState::Deciding => Self::Queue(Queued::Decision),
      PanelState::Ready => Self::FollowUp,
      PanelState::Failed => Self::Continue,
      PanelState::FailedWorktreeRemoved
      | PanelState::FailedNoAgent
      | PanelState::WorktreeRemoved
      | PanelState::Merged
      | PanelState::Ended => Self::Closed,
    }
  }

  /// Whom the run's state addresses unless the person chooses another: the agent that asked, whose
  /// plan waits, that a follow-up or a continuation resumes, or the agent at work; else Orchestrator.
  pub fn default_recipient(&self, conversation: &Conversation) -> Recipient {
    let by_agent = |agent: &str| {
      conversation
        .recipients
        .iter()
        .find(|choice| choice.agent.as_deref() == Some(agent))
        .map(|choice| choice.recipient)
    };
    self
      .fixed_recipient()
      .or_else(|| match self {
        Self::PlanChanges { agent, .. } => by_agent(agent),
        Self::FollowUp | Self::Continue => conversation.next_agent.as_deref().and_then(by_agent),
        _ => None,
      })
      .or_else(|| {
        conversation
          .working
          .first()
          .map(|working| Recipient::Agent(working.profile))
      })
      .unwrap_or(Recipient::Orchestrator)
  }

  /// What leaving this mode's context (its line's ×, or Esc) leaves: a question not answered or a
  /// plan's changes not requested, so the message is an ordinary one to `recipient`, which reads it
  /// at its next turn (`YJfPo`). Other modes have nothing to leave.
  pub fn without_context(self, conversation: &Conversation, recipient: Recipient) -> Self {
    if !matches!(self, Self::Answer { .. } | Self::PlanChanges { .. }) {
      return self;
    }
    match recipient {
      Recipient::Agent(profile) => Self::Queue(Queued::NextTurn(
        conversation
          .recipients
          .iter()
          .find(|choice| choice.recipient == Recipient::Agent(profile))
          .and_then(|choice| choice.agent.clone()),
      )),
      Recipient::Orchestrator => Self::Queue(Queued::Checkpoint),
    }
  }

  /// The recipient the composer is fixed to, if it is: the agent whose question it answers.
  pub const fn fixed_recipient(&self) -> Option<Recipient> {
    match self {
      Self::Answer {
        profile: Some(profile), ..
      } => Some(Recipient::Agent(*profile)),
      _ => None,
    }
  }
}

fn live(conversation: &Conversation, run: Option<&RunView>, recipient: Recipient) -> ComposerMode {
  if let Some(working) = conversation.working_for(recipient) {
    return ComposerMode::Steer(working.clone());
  }
  let tool_step = run.is_some_and(|run| run.working_steps().any(|step| step.tool_step));
  match recipient {
    Recipient::Agent(profile) => {
      let agent = conversation
        .recipients
        .iter()
        .find(|choice| choice.recipient == Recipient::Agent(profile))
        .and_then(|choice| choice.agent.clone());
      ComposerMode::Queue(Queued::NextTurn(agent))
    }
    Recipient::Orchestrator if tool_step => ComposerMode::Queue(Queued::ToolStep),
    Recipient::Orchestrator => ComposerMode::Queue(Queued::Checkpoint),
  }
}

/// A step waits for the person: a question's answer, else a plan's decision; a tool approval alone
/// leaves its agent at work, so the message queues for it.
fn waiting(conversation: &Conversation, run: Option<&RunView>) -> ComposerMode {
  if let Some(question) = conversation.open_question() {
    let agent = match &question.author {
      Author::Agent(agent) => agent.clone(),
      Author::You | Author::YouTo(_) => Arc::from(""),
    };
    let profile = conversation
      .recipients
      .iter()
      .find(|choice| choice.agent.as_ref() == Some(&agent))
      .and_then(|choice| match choice.recipient {
        Recipient::Agent(profile) => Some(profile),
        Recipient::Orchestrator => None,
      });
    return ComposerMode::Answer {
      agent,
      profile,
      question: question.id,
      asked_at: question.at,
    };
  }
  if let Some(review) = run.and_then(PlanReview::of) {
    return ComposerMode::PlanChanges {
      agent: review.agent.clone(),
      version: review.waiting().version,
    };
  }
  ComposerMode::Queue(Queued::Checkpoint)
}
