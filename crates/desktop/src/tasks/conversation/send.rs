//! Storing a person's message: a new one, sent now or after the turn, once per draft; and one sent
//! now that its agent's CLI did not take, sent after the turn instead. Both block on SQLite.

use std::path::Path;

use orchester_core::{
  conversation::{Message, MessageBody, MessageError, Recipient, Timing},
  id::{IdempotencyKey, MessageId, RunId, TaskId},
  run::EventSequence,
  store::ConversationStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::{self, Message as UiMessage};

/// Why a message was not sent.
#[derive(Debug)]
pub enum SendError {
  Blank,
  InvalidRecovery,
  /// The message is no longer one that was not delivered: it was sent again, or taken after all.
  NotUndelivered,
  Store(StoreError),
}

impl SendError {
  pub fn message(&self) -> UiMessage {
    match self {
      Self::Blank => UiMessage::new("conversation.errors.blank"),
      Self::InvalidRecovery => UiMessage::new("conversation.errors.invalidRecovery"),
      Self::NotUndelivered => UiMessage::new("conversation.errors.notUndelivered"),
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
  // A new message's text is only checked for being blank.
  let body = MessageBody::new(text).map_err(|_: MessageError| SendError::Blank)?;
  let message = Message::from_person(task, run, recipient, body, Timestamp::now())
    .sent(timing)
    .expect("a new message is a person's");
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
/// turn ends (*Send after this turn*).
pub fn send_after_turn_in(repo_root: &Path, task: TaskId, id: MessageId) -> Result<Message, SendError> {
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
