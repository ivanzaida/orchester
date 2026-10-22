//! Storing a person's message: a new one, sent now or after the turn, once per draft; and one sent
//! now that its agent's CLI did not take, sent after the turn instead, while its run is the task's latest
//! and an agent of it can still read it. Both block on SQLite.

use std::path::Path;

use orchester_core::{
  attachment::{NewAttachment, names_text},
  conversation::{Message, MessageBody, MessageError, Recipient, Sender, Timing},
  id::{ChatId, IdempotencyKey, MessageId, RunId, TaskId},
  run::{Answerer, EventSequence},
  store::ConversationStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::outgoing::{after_turn_run, answers_question, conversation_of};
use crate::{
  attachments::Refused,
  decisions::DecisionKind,
  messages::{self, Message as UiMessage},
  tasks::detail::TaskDetail,
};

/// Why a message was not sent.
#[derive(Debug)]
pub enum SendError {
  Blank,
  InvalidRecovery,
  /// The message is no longer one that was not delivered: it was sent again, or taken after all.
  NotUndelivered,
  /// The message's run ended or is no longer the task's latest: no agent reads it any more.
  NotReadable,
  /// Automation would decide for the person (answer a waiting question) without the consent for it.
  NotAllowed,
  /// Its attachments break the message's limits, or an agent it may reach could not read one.
  Attachments(Refused),
  Store(StoreError),
}

impl SendError {
  pub fn message(&self) -> UiMessage {
    match self {
      Self::Blank => UiMessage::new("conversation.errors.blank"),
      Self::InvalidRecovery => UiMessage::new("conversation.errors.invalidRecovery"),
      Self::NotUndelivered => UiMessage::new("conversation.errors.notUndelivered"),
      Self::NotReadable => UiMessage::new("conversation.errors.notReadable"),
      Self::NotAllowed => UiMessage::new("appMcp.errors.automationNotAllowed"),
      Self::Attachments(refused) => refused.message(),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

/// Stores a person's message `text` to `recipient`, sent as `timing` says, for `task`'s latest run
/// (`run`, if it has one), once per `key`: a retry after an unclear failure returns the message
/// stored the first time.
pub fn send_in(
  repo_root: &Path,
  key: IdempotencyKey,
  (task, run, recovery_after): (TaskId, Option<RunId>, Option<EventSequence>),
  (recipient, timing): (Recipient, Timing),
  text: &str,
) -> Result<Message, SendError> {
  send_as(
    repo_root,
    key,
    (task, run, recovery_after),
    (recipient, timing),
    ((text, &[]), None, Answerer::Person),
  )
}

/// [`send_in`] with `attachments`, decided `by`, and for the project's Orchestrator chat when `by_orchestrator` names
/// the conversation it came from: it wrote `text` on the person's side, and the conversation names it as the chat's.
/// A message of attachments only is kept with their names ([`names_text`]).
pub(super) fn send_as(
  repo_root: &Path,
  key: IdempotencyKey,
  (task, run, recovery_after): (TaskId, Option<RunId>, Option<EventSequence>),
  (recipient, timing): (Recipient, Timing),
  ((text, attachments), by_orchestrator, by): ((&str, &[NewAttachment]), Option<ChatId>, Answerer),
) -> Result<Message, SendError> {
  let named;
  let text = if text.trim().is_empty() && !attachments.is_empty() {
    named = names_text(attachments.iter().map(NewAttachment::name));
    named.as_str()
  } else {
    text
  };
  // A new message's text is only checked for being blank.
  let body = MessageBody::new(text).map_err(|_: MessageError| SendError::Blank)?;
  let message = Message::from_person(task, run, recipient, body, Timestamp::now())
    .sent(timing)
    .expect("a new message is a person's");
  let message = match by_orchestrator {
    Some(chat) => message
      .written_by_orchestrator(Some(chat))
      .expect("a new message is a person's"),
    None => message,
  };
  let message = if by == Answerer::Automation {
    message
      .sent_by_automation()
      .expect("a new message is on the person's side")
  } else {
    message
  };
  let message = match recovery_after {
    Some(sequence) => message
      .recovering_after(sequence)
      .map_err(|_| SendError::InvalidRecovery)?,
    None => message,
  };
  let mut db = ProjectDb::open(repo_root).map_err(SendError::Store)?;
  db.send_message_with(key, &message, attachments)
    .map_err(SendError::Store)
}

/// Queues `task`'s message `id`, which its agent's CLI did not take, again for when its agent's
/// turn ends (*Send after this turn*); refused once no agent of its run reads it ([`after_turn_run`]).
pub fn send_after_turn_in(repo_root: &Path, task: TaskId, id: MessageId) -> Result<Message, SendError> {
  send_after_turn_as(repo_root, (task, id), (Answerer::Person, |_| Ok(true)))
}

/// [`send_after_turn_in`], sent again `by`. A message queued again while its agent waits on a free-text question
/// answers it ([`answers_question`]): automation sends it only where `allowed` says automation may decide that kind
/// ([`SendError::NotAllowed`] otherwise). Automation's resend is recorded as automation's message.
pub fn send_after_turn_as(
  repo_root: &Path,
  (task, id): (TaskId, MessageId),
  (by, allowed): (Answerer, impl FnOnce(DecisionKind) -> Result<bool, StoreError>),
) -> Result<Message, SendError> {
  let detail = TaskDetail::load_in(repo_root, task).map_err(SendError::Store)?;
  let reads = conversation_of(&detail, false).state.takes_messages();
  if after_turn_run(&detail, reads, id).is_none() {
    return Err(SendError::NotReadable);
  }
  let mut db = ProjectDb::open(repo_root).map_err(SendError::Store)?;
  let stored = db.messages(task).map_err(SendError::Store)?;
  let before = stored
    .into_iter()
    .find(|message| message.id() == id)
    .ok_or(SendError::NotUndelivered)?;
  let Sender::Person { recipient, .. } = *before.sender() else {
    return Err(SendError::NotUndelivered);
  };
  if by == Answerer::Automation
    && answers_question(&detail, recipient)
    && !allowed(DecisionKind::Question).map_err(SendError::Store)?
  {
    return Err(SendError::NotAllowed);
  }
  let mut message = before.clone();
  message.send_after_turn().map_err(|_| SendError::NotUndelivered)?;
  if by == Answerer::Automation {
    message = message.sent_by_automation().map_err(|_| SendError::NotUndelivered)?;
  }
  // The runner may deliver it meanwhile, when the CLI takes it late after all: then it was not sent
  // again.
  if !db.change_delivery(&before, &message).map_err(SendError::Store)? {
    return Err(SendError::NotUndelivered);
  }
  Ok(message)
}
