//! Storing a person's message: a new one, sent now or after the turn, once per draft; and one sent
//! now that its agent's CLI did not take, sent after the turn instead, while its run is the task's latest
//! and an agent of it can still read it. Both block on SQLite.

use std::path::Path;

use orchester_core::{
  conversation::{Message, MessageBody, MessageError, Recipient, Timing},
  id::{IdempotencyKey, MessageId, RunId, TaskId},
  run::EventSequence,
  store::ConversationStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::outgoing::{after_turn_run, conversation_of};
use crate::{
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
  Store(StoreError),
}

impl SendError {
  pub fn message(&self) -> UiMessage {
    match self {
      Self::Blank => UiMessage::new("conversation.errors.blank"),
      Self::InvalidRecovery => UiMessage::new("conversation.errors.invalidRecovery"),
      Self::NotUndelivered => UiMessage::new("conversation.errors.notUndelivered"),
      Self::NotReadable => UiMessage::new("conversation.errors.notReadable"),
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
    (text, false),
  )
}

/// [`send_in`], for the project's Orchestrator chat when `by_orchestrator`: it wrote `text` on the person's side, and
/// the conversation names it as the chat's.
pub(super) fn send_as(
  repo_root: &Path,
  key: IdempotencyKey,
  (task, run, recovery_after): (TaskId, Option<RunId>, Option<EventSequence>),
  (recipient, timing): (Recipient, Timing),
  (text, by_orchestrator): (&str, bool),
) -> Result<Message, SendError> {
  // A new message's text is only checked for being blank.
  let body = MessageBody::new(text).map_err(|_: MessageError| SendError::Blank)?;
  let message = Message::from_person(task, run, recipient, body, Timestamp::now())
    .sent(timing)
    .expect("a new message is a person's");
  let message = if by_orchestrator {
    message.sent_by_orchestrator().expect("a new message is a person's")
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
  db.send_message(key, &message).map_err(SendError::Store)
}

/// Queues `task`'s message `id`, which its agent's CLI did not take, again for when its agent's
/// turn ends (*Send after this turn*); refused once no agent of its run reads it ([`after_turn_run`]).
pub fn send_after_turn_in(repo_root: &Path, task: TaskId, id: MessageId) -> Result<Message, SendError> {
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
  let mut message = before.clone();
  message.send_after_turn().map_err(|_| SendError::NotUndelivered)?;
  // The runner may deliver it meanwhile, when the CLI takes it late after all: then it was not sent
  // again.
  if !db.change_delivery(&before, &message).map_err(SendError::Store)? {
    return Err(SendError::NotUndelivered);
  }
  Ok(message)
}
