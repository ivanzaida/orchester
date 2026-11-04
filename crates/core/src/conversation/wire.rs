//! A message as it is read back from its serialized form (the coordinator link sends the task's messages to a run's
//! executor): checked as [`Message::restore`] checks a stored one, and only a message on the person's side carries the
//! markings the person's side has.

use serde::Deserialize;

use super::{Message, MessageBody, MessageError, Sender, Timing};
use crate::{
  id::{ChatId, MessageId, RunId, TaskId},
  run::EventSequence,
  time::Timestamp,
};

/// A message's fields as they were serialized, before they are checked.
#[derive(Deserialize)]
pub(super) struct MessageFields {
  id: MessageId,
  task: TaskId,
  run: Option<RunId>,
  sender: Sender,
  body: MessageBody,
  created_at: Timestamp,
  recovery_after: Option<EventSequence>,
  timing: Timing,
  by_orchestrator: bool,
  orchestrator_chat: Option<ChatId>,
  by_automation: bool,
}

impl TryFrom<MessageFields> for Message {
  type Error = MessageError;

  fn try_from(fields: MessageFields) -> Result<Self, MessageError> {
    let mut message = Self::restore(
      fields.id,
      fields.task,
      fields.run,
      fields.sender,
      fields.body,
      fields.created_at,
    )?;
    let marked = fields.recovery_after.is_some()
      || fields.timing != Timing::AfterTurn
      || fields.by_orchestrator
      || fields.orchestrator_chat.is_some()
      || fields.by_automation;
    if !marked {
      return Ok(message);
    }
    if !matches!(message.sender, Sender::Person { .. }) {
      return Err(MessageError::NotFromAPerson);
    }
    if fields.recovery_after.is_some() && message.run.is_none() {
      return Err(MessageError::WithoutRun);
    }
    message.recovery_after = fields.recovery_after;
    message.timing = fields.timing;
    message.by_orchestrator = fields.by_orchestrator;
    message.orchestrator_chat = fields.orchestrator_chat;
    message.by_automation = fields.by_automation;
    Ok(message)
  }
}
