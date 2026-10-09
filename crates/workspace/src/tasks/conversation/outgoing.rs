//! A person's message written now, prepared and stored the same way for the Conversation tab's composer and
//! for automation (`send_message`): the task is read again when the message is sent, so a message is only
//! ever for the task's latest run as it stands then, and what the run's state offers ([`ComposerMode`]) is
//! decided from that read, not from what a screen last showed. A plan's changes are a decision on the plan,
//! told to the runner as a command; every other mode stores the message once per key, then what its
//! action says follows ([`super::follow`]).

use std::path::Path;

use orchester_core::{
  conversation::{Message, Recipient, Timing},
  id::{IdempotencyKey, RunId, TaskId},
  integration::Leftovers,
  run::{Answerer, PlanDecision, ReviewNote},
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

/// Why a message written now was not stored.
#[derive(Debug)]
pub enum OutgoingError {
  /// The task's latest run is no longer the one the message was written for: `latest`, by its number,
  /// or none that can be read.
  NotLatestRun(Option<u32>),
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
      Self::Closed(state) => UiMessage::new(state.closed_reason()),
      Self::NoSecondWay => UiMessage::new("conversation.errors.noSecondWay"),
      Self::NoPlan => UiMessage::new("runs.plan.errors.notAwaited"),
      Self::Send(error) => error.message(),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

/// What the run's state offers for a message to the recipient it addresses: the mode, and the recipient.
/// `left_context`: the person left the question's or the plan's mode the state offers, so the message is
/// an ordinary one.
pub fn offer(conversation: &Conversation, run: Option<&RunView>, left_context: bool) -> (ComposerMode, Recipient) {
  let recipient = ComposerMode::of(conversation, run, Recipient::Orchestrator).default_recipient(conversation);
  let offered = ComposerMode::of(conversation, run, recipient);
  let mode = if left_context {
    offered.without_context(conversation, recipient)
  } else {
    offered
  };
  (mode, recipient)
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
}

/// Reads `task` again for a message written for its latest run `written_for` (`None`: before it had one);
/// `lost`: the run is recorded as running while no runner looks after it; `left_context` as in [`offer`].
/// A recorded removal of the run's worktree counts as the worktree gone. Blocks on SQLite.
pub fn prepare_in(
  root: &Path,
  (task, written_for): (TaskId, Option<RunId>),
  (lost, left_context): (bool, bool),
) -> Result<Prepared, OutgoingError> {
  let detail = TaskDetail::load_in(root, task).map_err(OutgoingError::Store)?;
  let latest = detail.run.as_ref();
  if latest.map(RunView::id) != written_for {
    let number = latest.map(|run| run.run.number().get());
    return Err(OutgoingError::NotLatestRun(number));
  }
  let removed = latest
    .and_then(|run| run.worktree_removed)
    .map(|_| Leftovers::default());
  let conversation = Conversation::of(&detail, lost, removed.as_ref());
  let (mode, recipient) = offer(&conversation, latest, left_context);
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
  let timing = match (&prepared.mode, way) {
    (ComposerMode::Steer(working), Way::Primary) if working.mid_turn => Timing::Now,
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
/// tell the runner.
pub fn plan_changes(prepared: &Prepared, text: &str, by: Answerer) -> Result<Command, OutgoingError> {
  let review = prepared
    .detail
    .run
    .as_ref()
    .and_then(PlanReview::of)
    .ok_or(OutgoingError::NoPlan)?;
  let note = ReviewNote::new(text).map_err(|_| OutgoingError::Send(SendError::Blank))?;
  let waiting = review.waiting();
  Ok(Command::DecidePlan {
    run: review.run,
    execution: waiting.execution,
    version: waiting.version,
    decision: PlanDecision::ChangesRequested { note },
    by,
  })
}
