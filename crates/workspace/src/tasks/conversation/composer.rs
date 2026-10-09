//! What the conversation's composer offers in the run's state (`docs/design/orchester-workspace.md`,
//! "Conversation (merged with Activity)", Composer, and "The chat always answers" (ORC-79)): while an
//! agent works and the message is for it, *Send now* and *Send after this turn*; a question
//! answered, a plan's changes requested, a follow-up asked, a failed or stuck run continued with
//! instructions, a paused or interrupted run resumed with the message, a run stopped at its limit
//! raised, an ended run run again with the message, a message queued for a run that has not started
//! or does not work now; or nothing, once no agent reads messages and none can again. Every message
//! the composer stores has an agent that reads it, and the composer says which and when.

use std::sync::Arc;

use orchester_core::{
  conversation::Recipient,
  id::{MessageId, ResourceId},
  run::{PauseReason, PlanVersion, RunLimit, RunStatus, can_run_again_after},
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
  /// The message queues: a tool step runs, the recipient does not work now, the run stopped at a
  /// limit the person does not raise here, or it waits for a decision no agent can be continued from.
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
  /// The run failed, or waits for the person after a step's retries ran out or it broke again: the
  /// message continues it with instructions, from the step of the agent it is for.
  Continue,
  /// The run is paused (`interrupted`: it lost its runner): *Resume and send* continues it and its
  /// agent reads the message before its next action; the message can also wait until it does.
  Resume { interrupted: bool },
  /// The run stopped at a limit of its own, which a person raises: the message waits for the run to
  /// continue, and *Raise the limit and continue* opens where the limit is raised.
  RaiseLimit,
  /// The run has not started: the message waits for its first step.
  NotStarted,
  /// The run ended and none of its agents reads messages any more: *Run again with this message*
  /// starts the task's next run, whose first agent reads it.
  RunAgain,
  /// No agent reads messages any more, and the task does not run again.
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
  /// The run stopped at a budget it shares with other runs, the project's daily budget or its task
  /// group's: it continues once that budget has room.
  SharedBudget(RunLimit),
  /// The run waits for the person's decision on its Summary, after a step's retries ran out or it
  /// broke again, with no agent step to continue from: the message waits until the run continues.
  Decision,
  /// Agents work, but none is at a turn yet.
  Checkpoint,
  /// Agent `0` asked a question with suggested answers, which its card answers: the message reaches it
  /// after the answer, and does not answer the question.
  AnswerAbove(Arc<str>),
}

/// What follows once the composer stored the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
  /// The runner is told: it delivers the message at the run's next boundary, resumes the step that
  /// asked, follows the run up or continues it from the step it stopped at.
  Deliver,
  /// The paused run resumes; its agent reads the message before its next action.
  Resume,
  /// The interrupted run continues from its last checkpoint; its agent reads the message first.
  ContinueInterrupted,
  /// The message waits for the run to continue; the person raises the limit on the task's Summary.
  RaiseLimit,
  /// The task's next run starts; its first agent reads the message.
  RunAgain,
}

impl ComposerMode {
  /// The mode for a message to `recipient` in `conversation` of `run`.
  pub fn of(conversation: &Conversation, run: Option<&RunView>, recipient: Recipient) -> Self {
    if let Some(asker) = asking(conversation, run, recipient) {
      return Self::Queue(Queued::AnswerAbove(asker));
    }
    match conversation.state {
      PanelState::NotStarted => Self::NotStarted,
      PanelState::Live => live(conversation, run, recipient),
      PanelState::Waiting => waiting(conversation, run),
      PanelState::Paused => Self::Resume { interrupted: false },
      PanelState::Interrupted if run.is_some_and(|run| run.worktree_removed.is_some()) => Self::RunAgain,
      PanelState::Interrupted => Self::Resume { interrupted: true },
      PanelState::Stopped => match run.map(|run| run.run.status()) {
        Some(RunStatus::Paused(PauseReason::LimitReached(
          shared @ (RunLimit::DailyBudget | RunLimit::GroupBudget),
        ))) => Self::Queue(Queued::SharedBudget(*shared)),
        _ => Self::RaiseLimit,
      },
      PanelState::Deciding if conversation.recovery_after.is_some() => Self::Continue,
      PanelState::Deciding => Self::Queue(Queued::Decision),
      PanelState::Ready => Self::FollowUp,
      PanelState::Failed => Self::Continue,
      PanelState::FailedWorktreeRemoved | PanelState::FailedNoAgent | PanelState::Ended
        if run.is_some_and(|run| can_run_again_after(&run.run, run.worktree_removed.is_some())) =>
      {
        Self::RunAgain
      }
      PanelState::FailedWorktreeRemoved
      | PanelState::FailedNoAgent
      | PanelState::WorktreeRemoved
      | PanelState::Merged
      | PanelState::Ended => Self::Closed,
    }
  }

  /// Whom the run's state addresses unless the person chooses another: the agent that asked, whose
  /// plan waits, that a follow-up or a continuation resumes, that reads first once a suspended run
  /// goes on, or the agent at work; else Orchestrator.
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
        Self::PlanChanges { agent, .. } | Self::Queue(Queued::AnswerAbove(agent)) => by_agent(agent),
        Self::FollowUp
        | Self::Continue
        | Self::Resume { .. }
        | Self::RaiseLimit
        | Self::Queue(Queued::SharedBudget(_)) => conversation.next_agent.as_deref().and_then(by_agent),
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

  /// What follows the stored message when the person presses the primary action; `None` where the
  /// composer stores nothing: a plan's changes are a decision on the plan, and a closed composer
  /// sends nothing.
  pub const fn then(&self) -> Option<Then> {
    match self {
      Self::Resume { interrupted: false } => Some(Then::Resume),
      Self::Resume { interrupted: true } => Some(Then::ContinueInterrupted),
      Self::RaiseLimit => Some(Then::RaiseLimit),
      Self::RunAgain => Some(Then::RunAgain),
      Self::PlanChanges { .. } | Self::Closed => None,
      Self::Steer(_) | Self::Queue(_) | Self::Answer { .. } | Self::FollowUp | Self::Continue | Self::NotStarted => {
        Some(Then::Deliver)
      }
    }
  }

  /// What the second way of sending does, where the mode has one: a message for an agent at work
  /// that takes one mid-turn waits for its turn to end; a paused or interrupted run's message waits
  /// for it to continue, without continuing it.
  pub const fn then_later(&self) -> Option<Then> {
    match self {
      Self::Steer(working) if working.mid_turn => Some(Then::Deliver),
      Self::Resume { .. } => Some(Then::Deliver),
      _ => None,
    }
  }
}

impl Then {
  /// Whether the message is kept for the task's next run rather than for its latest one, which has
  /// ended.
  pub const fn for_next_run(self) -> bool {
    matches!(self, Self::RunAgain)
  }
}

/// The agent whose question with suggested answers waits on its card, while a message to `recipient`
/// reaches it: written to it or to Orchestrator.
fn asking(conversation: &Conversation, run: Option<&RunView>, recipient: Recipient) -> Option<Arc<str>> {
  if !matches!(conversation.state, PanelState::Live | PanelState::Waiting) {
    return None;
  }
  let asker: Arc<str> = run?.waiting_questions.first()?.request.agent().into();
  let reaches = match recipient {
    Recipient::Orchestrator => true,
    Recipient::Agent(profile) => conversation
      .recipients
      .iter()
      .any(|choice| choice.recipient == Recipient::Agent(profile) && choice.agent.as_ref() == Some(&asker)),
  };
  reaches.then_some(asker)
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
