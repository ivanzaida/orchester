//! A person's message written now, prepared and stored the same way for the Conversation tab's composer and
//! for automation (`send_message`): the task is read again when the message is sent, so a message is only
//! ever for the task's latest run as it stands then, and what the run's state offers ([`ComposerMode`]) is
//! decided from that read, not from what a screen last showed. A person sends what they saw: the action
//! they pressed ([`Action`]) is checked against the one the read offers, and a different one is refused
//! ([`OutgoingError::Changed`]) rather than done; automation takes whatever is offered. A plan's changes are
//! a decision on the plan, told to the runner as a command; every other mode stores the message once per
//! key, then what its action says follows ([`super::follow`]).

use std::path::Path;

use orchester_core::{
  conversation::{Message, Recipient, Timing},
  id::{IdempotencyKey, MessageId, RunId, TaskId},
  integration::Leftovers,
  run::{Answerer, PlanDecision, PlanVersion, ReviewNote},
};
use orchester_ipc::Command;
use orchester_store::StoreError;

use super::{
  Conversation, PanelState, SendError,
  composer::{ComposerMode, Then},
  send_in,
};
use crate::{
  messages::{self, Message as UiMessage},
  tasks::{
    detail::{RunView, TaskDetail},
    plan::PlanReview,
  },
};

/// Which of the mode's ways of sending: its own (Enter, the send button), or the second it offers
/// (Ctrl/Cmd+Enter, the send menu): *Send after this turn* while an agent works, *Send without resuming*
/// while the run is paused or interrupted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
  Primary,
  Second,
}

/// What a mode answers or changes, which its context line names and the person can leave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
  Question(MessageId),
  Plan(PlanVersion),
}

impl Context {
  pub fn of(mode: &ComposerMode) -> Option<Self> {
    match mode {
      ComposerMode::Answer { question, .. } => Some(Self::Question(*question)),
      ComposerMode::PlanChanges { version, .. } => Some(Self::Plan(*version)),
      _ => None,
    }
  }
}

/// What sending does, as the person pressed it: two presses that do different things never pass for each
/// other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
  /// To the agent at work, at its next tool call.
  SendNow,
  /// Stored for the runner to deliver to `0` at the run's next boundary, or when its run starts.
  Deliver(Recipient),
  /// The answer to the question.
  Answer(MessageId),
  /// The changes plan `0` needs.
  PlanChanges(PlanVersion),
  /// A follow-up: the agent of the run that is ready for review works again.
  FollowUp,
  /// Continues the failed or stuck run from an agent's work.
  Continue,
  Resume,
  ContinueInterrupted,
  /// Kept until the person raises the limit the run stopped at.
  RaiseLimit,
  /// Starts the task's next run.
  RunAgain,
}

impl Action {
  /// What sending `way` to `to` does in `mode`; `None` where it sends nothing.
  pub fn of(mode: &ComposerMode, way: Way, to: Recipient) -> Option<Self> {
    let then = match way {
      Way::Primary => mode.then(),
      Way::Second => mode.then_later(),
    };
    let action = match (mode, way) {
      (ComposerMode::PlanChanges { version, .. }, Way::Primary) => return Some(Self::PlanChanges(*version)),
      (ComposerMode::Steer(working), Way::Primary) if working.mid_turn => Self::SendNow,
      (ComposerMode::Answer { question, .. }, Way::Primary) => Self::Answer(*question),
      (ComposerMode::FollowUp, Way::Primary) => Self::FollowUp,
      (ComposerMode::Continue, Way::Primary) => Self::Continue,
      _ => match then? {
        Then::Deliver => Self::Deliver(to),
        Then::Resume => Self::Resume,
        Then::ContinueInterrupted => Self::ContinueInterrupted,
        Then::RaiseLimit => Self::RaiseLimit,
        Then::RunAgain => Self::RunAgain,
      },
    };
    Some(action)
  }
}

/// Why a message written now was not stored.
#[derive(Debug)]
pub enum OutgoingError {
  /// The task's latest run is no longer the one the message was written for: `latest`, by its number,
  /// or none that can be read.
  NotLatestRun(Option<u32>),
  /// The run's state changed since the person saw it: sending would do something else than they pressed.
  Changed,
  /// No agent reads messages any more, and the task does not run again, as the state says.
  Closed(PanelState),
  /// The mode has one way of sending only.
  NoSecondWay,
  /// A plan's changes were asked for, but no plan waits for a decision.
  NoPlan,
  Send(SendError),
  Store(StoreError),
}

impl OutgoingError {
  pub fn message(&self) -> UiMessage {
    match self {
      Self::NotLatestRun(Some(latest)) => {
        UiMessage::new("conversation.errors.notLatestRun").with("run", latest.to_string())
      }
      Self::NotLatestRun(None) => UiMessage::new("runs.unreadable.rejected"),
      Self::Changed => UiMessage::new("conversation.errors.runChanged"),
      Self::Closed(state) => UiMessage::new(state.closed_reason()),
      Self::NoSecondWay => UiMessage::new("conversation.errors.noSecondWay"),
      Self::NoPlan => UiMessage::new("runs.plan.errors.notAwaited"),
      Self::Send(error) => error.message(),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

/// The conversation of `detail` when a message is sent; `lost`: its run is recorded as running while no
/// runner looks after it. A recorded removal of the run's worktree counts as the worktree gone.
pub fn conversation_of(detail: &TaskDetail, lost: bool) -> Conversation {
  let removed = detail
    .run
    .as_ref()
    .and_then(|run| run.worktree_removed)
    .map(|_| Leftovers::default());
  Conversation::of(detail, lost, removed.as_ref())
}

/// What the run's state offers for a message to the recipient it addresses: the mode, and the recipient.
/// `left`: the question's or the plan's mode the person left; while the state offers it, the message is an
/// ordinary one.
pub fn offer(conversation: &Conversation, run: Option<&RunView>, left: Option<Context>) -> (ComposerMode, Recipient) {
  let recipient = ComposerMode::of(conversation, run, Recipient::Orchestrator).default_recipient(conversation);
  let offered = ComposerMode::of(conversation, run, recipient);
  let context = Context::of(&offered);
  let mode = if context.is_some() && context == left {
    offered.without_context(conversation, recipient)
  } else {
    offered
  };
  (mode, recipient)
}

/// The run a message its agent's CLI did not take is queued again for (*Send after this turn*): its own,
/// while it is the task's latest and an agent of it can still read messages (`reads`).
pub fn after_turn_run(detail: &TaskDetail, reads: bool, message: MessageId) -> Option<RunId> {
  let latest = detail.run.as_ref()?.id();
  let run = detail.messages.iter().find(|stored| stored.id() == message)?.run()?;
  (run == latest && reads).then_some(run)
}

/// The task as read when its message is sent, and what its latest run's state offers.
pub struct Prepared {
  pub detail: TaskDetail,
  pub conversation: Conversation,
  pub mode: ComposerMode,
  pub recipient: Recipient,
}

impl Prepared {
  /// The task's latest run.
  pub fn run(&self) -> Option<RunId> {
    self.detail.run.as_ref().map(RunView::id)
  }

  /// Refuses sending `way` unless it does what the person pressed, `expected`.
  pub fn check(&self, way: Way, expected: Action) -> Result<(), OutgoingError> {
    if Action::of(&self.mode, way, self.recipient) == Some(expected) {
      return Ok(());
    }
    Err(OutgoingError::Changed)
  }
}

/// Reads `task` again for a message written for its latest run `written_for` (`None`: before it had one);
/// `lost` as in [`conversation_of`], `left` as in [`offer`]. Blocks on SQLite.
pub fn prepare_in(
  root: &Path,
  (task, written_for): (TaskId, Option<RunId>),
  (lost, left): (bool, Option<Context>),
) -> Result<Prepared, OutgoingError> {
  let detail = TaskDetail::load_in(root, task).map_err(OutgoingError::Store)?;
  let latest = detail.run.as_ref();
  if latest.map(RunView::id) != written_for {
    let number = latest.map(|run| run.run.number().get());
    return Err(OutgoingError::NotLatestRun(number));
  }
  let conversation = conversation_of(&detail, lost);
  let (mode, recipient) = offer(&conversation, latest, left);
  Ok(Prepared {
    detail,
    conversation,
    mode,
    recipient,
  })
}

/// Stores `text` as the message `prepared` offers, sent `way`, once per `key`; with what follows it. A
/// message for the task's next run names no run; only a message that continues the run carries the
/// failure it continues after. Blocks on SQLite.
pub fn store_in(
  root: &Path,
  prepared: &Prepared,
  (key, way): (IdempotencyKey, Way),
  text: &str,
) -> Result<(Then, Message), OutgoingError> {
  let then = match way {
    Way::Primary => prepared.mode.then(),
    Way::Second => prepared.mode.then_later(),
  };
  let then = match (then, way) {
    (Some(then), _) => then,
    (None, Way::Second) => return Err(OutgoingError::NoSecondWay),
    (None, Way::Primary) => return Err(OutgoingError::Closed(prepared.conversation.state)),
  };
  let timing = match Action::of(&prepared.mode, way, prepared.recipient) {
    Some(Action::SendNow) => Timing::Now,
    _ => Timing::AfterTurn,
  };
  let run = prepared.run().filter(|_| !then.for_next_run());
  let recovery_after = prepared
    .conversation
    .recovery_after
    .filter(|_| prepared.mode == ComposerMode::Continue);
  let task = prepared.detail.task.id();
  let stored = send_in(
    root,
    key,
    (task, run, recovery_after),
    (prepared.recipient, timing),
    text,
  )
  .map_err(OutgoingError::Send)?;
  Ok((then, stored))
}

/// *Request changes* with `text` as the changes the plan that waits needs, decided `by`: the command to
/// tell the runner. `version`, where given, is the plan the person saw: another one waiting is refused.
pub fn plan_changes(
  prepared: &Prepared,
  text: &str,
  (by, version): (Answerer, Option<PlanVersion>),
) -> Result<Command, OutgoingError> {
  let review = prepared
    .detail
    .run
    .as_ref()
    .and_then(PlanReview::of)
    .ok_or(OutgoingError::NoPlan)?;
  let waiting = review.waiting();
  if version.is_some_and(|seen| seen != waiting.version) {
    return Err(OutgoingError::Changed);
  }
  let note = ReviewNote::new(text).map_err(|_| OutgoingError::Send(SendError::Blank))?;
  Ok(Command::DecidePlan {
    run: review.run,
    execution: waiting.execution,
    version: waiting.version,
    decision: PlanDecision::ChangesRequested { note },
    by,
  })
}
