//! Writing to the Orchestrator while it answers (ORC-92, `docs/design/orchestrator-toolset.md`, "Writing while it
//! answers"): a person's message kept while a turn of its conversation is under way waits for it
//! ([`TurnState::Queued`]), or is handed to it ([`TurnState::Steering`]) and taken by it ([`TurnState::Joined`]). A
//! queued message gets its turn once no turn of its conversation thinks and none holds a message sent now; it moves to
//! the end of its conversation then, so the answer under way keeps what it said before. Until then the person may
//! remove it (the app's *Edit* reads its files back first, and takes them and its words into the composer).

use orchester_core::{
  attachment::NewAttachment,
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, Queued, TurnState},
  run::Answerer,
  time::Timestamp,
};
use rusqlite::{Transaction, params};

use super::{
  ProjectDb,
  attachments::{self, AttachmentOwner},
  orchestrator::{entries_in, failure_column},
};
use crate::{StoreError, rows::parse};

/// The words of the states a person's message waits for a turn in, which the person may still change.
const WAITING: [&str; 3] = ["queued", "queued_next", "not_taken"];

/// A queued message whose turn starts now, and its conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartedQueued {
  pub chat: ChatId,
  pub message: ChatMessageId,
}

impl ProjectDb {
  /// Keeps the person's message `message` of `chat` (stored with it when it is new), with `attachments`, sent `by` the
  /// person or by automation driving the window, its turn as `state` says at `at`: thinking for a turn of its own now,
  /// queued or handed to the turn under way. Returns whether it was new: a message kept before is never kept twice,
  /// whatever its turn did since.
  pub fn keep_chat_message_as(
    &mut self,
    chat: &Chat,
    (message, text): (ChatMessageId, &MessageBody),
    (attachments, by): (&[NewAttachment], Answerer),
    (state, at): (&TurnState, Timestamp),
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT OR IGNORE INTO orchestrator_chats (id, title, started_at) VALUES (?1, ?2, ?3)",
      params![chat.id().to_string(), chat.title(), chat.started().unix_millis()],
    )?;
    let kept = transaction.execute(
      "INSERT OR IGNORE INTO orchestrator_entries (chat_id, kind, at, body, message_id, turn, failure, by_automation)
       VALUES (?1, 'person', ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        chat.id().to_string(),
        at.unix_millis(),
        text.as_str(),
        message.to_string(),
        state.word(),
        failure_column(state)?,
        by == Answerer::Automation
      ],
    )?;
    if kept == 0 {
      transaction.commit()?;
      return Ok(false);
    }
    let (written, attached) = attachments::write(&self.directory, attachments)?;
    attachments::insert(&transaction, AttachmentOwner::ChatMessage(message), &attached, at)?;
    transaction.commit()?;
    written.keep();
    Ok(true)
  }

  /// Changes the turn of the person's message `message` to `to` while it is one of `from`; whether it changed.
  pub fn change_chat_turn(
    &mut self,
    message: ChatMessageId,
    from: &[TurnState],
    to: &TurnState,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let mut changed = false;
    for state in from {
      changed |= transaction.execute(
        "UPDATE orchestrator_entries SET turn = ?3, failure = ?4
         WHERE message_id = ?1 AND kind = 'person' AND turn = ?2",
        params![message.to_string(), state.word(), to.word(), failure_column(to)?],
      )? == 1;
    }
    transaction.commit()?;
    Ok(changed)
  }

  /// Leaves the messages of `chat` still handed to its turn, which ended without taking them, for turns of their own
  /// ([`Queued::NotTaken`]); the messages it left so.
  pub fn release_chat_steering(&mut self, chat: ChatId) -> Result<Vec<ChatMessageId>, StoreError> {
    let transaction = self.connection.transaction()?;
    let released: Vec<ChatMessageId> = entries_in(&transaction, "steering")?
      .into_iter()
      .filter(|(_, of)| *of == chat)
      .map(|(message, _)| message)
      .collect();
    for message in &released {
      set_not_taken(&transaction, *message)?;
    }
    transaction.commit()?;
    Ok(released)
  }

  /// Starts the turn of the next queued message of a conversation where no turn thinks and no message is handed to a
  /// turn: a message of *Stop and send* before the others, else the oldest. It moves to the end of its conversation.
  /// `None` when no queued message can start.
  pub fn start_queued_chat_turn(&mut self) -> Result<Option<StartedQueued>, StoreError> {
    let transaction = self.connection.transaction()?;
    // In the order they wait: a message of Stop and send first, then by when kept.
    let queued: Vec<(ChatMessageId, ChatId)> = {
      let mut statement = transaction.prepare(
        "SELECT message_id, chat_id FROM orchestrator_entries WHERE turn IN ('queued', 'queued_next', 'not_taken')
         ORDER BY turn = 'queued_next' DESC, seq",
      )?;
      statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .map(|row| {
          let (message, chat) = row?;
          Ok((parse("chat message id", &message)?, parse("chat id", &chat)?))
        })
        .collect::<Result<_, StoreError>>()?
    };
    for (message, chat) in queued {
      let started = transaction.execute(
        "UPDATE orchestrator_entries SET turn = 'thinking', seq = (SELECT MAX(seq) + 1 FROM orchestrator_entries)
         WHERE message_id = ?1 AND turn IN ('queued', 'queued_next', 'not_taken')
           AND NOT EXISTS (SELECT 1 FROM orchestrator_entries WHERE chat_id = ?2 AND turn IN ('thinking', 'steering'))",
        params![message.to_string(), chat.to_string()],
      )?;
      if started == 1 {
        transaction.commit()?;
        return Ok(Some(StartedQueued { chat, message }));
      }
    }
    transaction.commit()?;
    Ok(None)
  }

  /// Whether a queued message could start its turn now ([`Self::start_queued_chat_turn`]), without starting it.
  pub fn queued_chat_turn_ready(&self) -> Result<bool, StoreError> {
    let ready: i64 = self.connection.query_row(
      "SELECT EXISTS (SELECT 1 FROM orchestrator_entries e WHERE e.turn IN ('queued', 'queued_next', 'not_taken')
         AND NOT EXISTS (SELECT 1 FROM orchestrator_entries o WHERE o.chat_id = e.chat_id
           AND o.turn IN ('thinking', 'steering')))",
      [],
      |row| row.get(0),
    )?;
    Ok(ready == 1)
  }

  /// Removes the queued message `message` with its attachments; whether it still waited.
  pub fn remove_queued_chat_message(&mut self, message: ChatMessageId) -> Result<bool, StoreError> {
    self.forget_person(message, &WAITING)
  }
}

/// Leaves the messages handed to a turn that no runner answers for turns of their own: each of a conversation none of
/// whose thinking turns is `held`, of conversation `only` when it names one. Returns their conversations.
pub(super) fn release_unheld_steering(
  transaction: &Transaction<'_>,
  held: &[ChatMessageId],
  only: Option<ChatId>,
) -> Result<Vec<ChatId>, StoreError> {
  let held_chats: Vec<ChatId> = entries_in(transaction, "thinking")?
    .into_iter()
    .filter(|(message, _)| held.contains(message))
    .map(|(_, chat)| chat)
    .collect();
  let mut chats = Vec::new();
  for (message, chat) in entries_in(transaction, "steering")? {
    if held_chats.contains(&chat) || only.is_some_and(|only| only != chat) {
      continue;
    }
    set_not_taken(transaction, message)?;
    if !chats.contains(&chat) {
      chats.push(chat);
    }
  }
  Ok(chats)
}

fn set_not_taken(transaction: &Transaction<'_>, message: ChatMessageId) -> Result<(), StoreError> {
  transaction.execute(
    "UPDATE orchestrator_entries SET turn = ?2 WHERE message_id = ?1 AND turn = 'steering'",
    params![message.to_string(), TurnState::Queued(Queued::NotTaken).word()],
  )?;
  Ok(())
}
