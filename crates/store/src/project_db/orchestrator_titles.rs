//! What a person changes of an Orchestrator conversation in History: its title, which wins over the one taken from
//! its first message from then on, and its pin to History's top (ORC-328); its archive, which lists it under Archived
//! instead, and its deletion (ORC-329).
//!
//! Deleting a conversation removes it, every entry it holds (the person's messages, the agent's replies, its actions,
//! events, waits and reports, with the todo lists of its turns) and the attachments of its messages: an attachment
//! row belongs to exactly one message (`attachments.chat_message_id`) and its copy is named by the attachment's own
//! id, so the rows found by the conversation's messages and their copies are the ones it alone held. Its
//! subscriptions end, so their targets are free to lead again (ORC-758). What its turns spent stays counted, and the
//! tasks, runs and decisions it started stay as they are.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::ChatTitle,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{
  ProjectDb,
  attachments::{self, AttachmentOwner},
  orchestrator_subscriptions::end_chat_subscriptions,
};
use crate::{StoreError, rows::parse};

/// What became of a person's deletion of a conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatDeletion {
  /// It is gone with everything it held.
  Deleted,
  /// The project no longer kept it.
  Gone,
  /// Nothing was deleted: the Orchestrator answers in it, compacts its session, or a message waits there for its turn.
  Busy,
}

impl ProjectDb {
  /// Titles conversation `chat` `title` as a person named it. Returns whether the project keeps the conversation.
  pub fn rename_chat(&mut self, chat: ChatId, title: &ChatTitle) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE orchestrator_chats SET person_title = ?2 WHERE id = ?1",
      params![chat.to_string(), title.as_str()],
    )?;
    Ok(changed == 1)
  }

  /// Pins conversation `chat` to History's top as pinned at `at`, or unpins it (`None`). Returns whether the project
  /// keeps the conversation.
  pub fn pin_chat(&mut self, chat: ChatId, at: Option<Timestamp>) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE orchestrator_chats SET pinned_at = ?2 WHERE id = ?1",
      params![chat.to_string(), at.map(|at| at.unix_millis())],
    )?;
    Ok(changed == 1)
  }

  /// Archives conversation `chat` as of `at`, which takes it out of History's list and its pin, or unarchives it
  /// (`None`), which lists it under the day it last changed again. Returns whether the project keeps the conversation.
  pub fn archive_chat(&mut self, chat: ChatId, at: Option<Timestamp>) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE orchestrator_chats SET archived_at = ?2, pinned_at = CASE WHEN ?2 IS NULL THEN pinned_at END
       WHERE id = ?1",
      params![chat.to_string(), at.map(|at| at.unix_millis())],
    )?;
    Ok(changed == 1)
  }

  /// Deletes conversation `chat` with every entry it holds and the attachments of its messages, and ends its
  /// subscriptions now, in one transaction, then removes their copies; nothing while it is busy
  /// ([`ChatDeletion::Busy`]).
  pub fn delete_chat(&mut self, chat: ChatId) -> Result<ChatDeletion, StoreError> {
    let id = chat.to_string();
    let transaction = self.connection.transaction()?;
    match busy(&transaction, chat)? {
      None => return Ok(ChatDeletion::Gone),
      Some(true) => return Ok(ChatDeletion::Busy),
      Some(false) => {}
    }
    let copies = forget_attachments(&transaction, &self.directory, chat, 0)?;
    transaction.execute("DELETE FROM orchestrator_entries WHERE chat_id = ?1", [&id])?;
    transaction.execute("DELETE FROM orchestrator_chats WHERE id = ?1", [&id])?;
    end_chat_subscriptions(&transaction, chat, Timestamp::now())?;
    transaction.commit()?;
    attachments::remove_copies(&copies);
    Ok(ChatDeletion::Deleted)
  }
}

/// Whether conversation `chat` is busy: the Orchestrator answers in it, compacts its session, or a message waits there
/// for its turn; `None` when the project does not keep it.
pub(super) fn busy(connection: &Connection, chat: ChatId) -> Result<Option<bool>, StoreError> {
  let busy: Option<i64> = connection
    .query_row(
      "SELECT EXISTS (SELECT 1 FROM orchestrator_entries WHERE chat_id = ?1
         AND (turn IN ('thinking', 'steering', 'queued', 'queued_next', 'not_taken')
           OR (kind = 'event' AND json_extract(body, '$.kind') = 'compaction'
             AND json_extract(body, '$.state.state') = 'running')))
       FROM orchestrator_chats WHERE id = ?1",
      [chat.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  Ok(busy.map(|busy| busy != 0))
}

/// Forgets the attachments of the messages of `chat` kept as entry `from` or after it, whose copies in `directory`
/// are returned to be removed once the change is committed.
pub(super) fn forget_attachments(
  connection: &Connection,
  directory: &Path,
  chat: ChatId,
  from: i64,
) -> Result<Vec<PathBuf>, StoreError> {
  let messages: Vec<String> = connection
    .prepare("SELECT message_id FROM orchestrator_entries WHERE chat_id = ?1 AND seq >= ?2 AND message_id IS NOT NULL")?
    .query_map(params![chat.to_string(), from], |row| row.get(0))?
    .collect::<Result<_, _>>()?;
  let mut copies = Vec::new();
  for message in messages {
    let message: ChatMessageId = parse("chat message id", &message)?;
    copies.extend(attachments::forget(
      connection,
      directory,
      AttachmentOwner::ChatMessage(message),
    )?);
  }
  Ok(copies)
}
