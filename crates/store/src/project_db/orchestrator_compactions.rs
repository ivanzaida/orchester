//! A person's compactions of an Orchestrator conversation's session (*Compact*, ORC-309): each is an event of the
//! conversation ([`ChatEvent::Compaction`]), kept running when the runner starts it and changed in place once it ended.
//! A compaction that ended takes the session's reading with it: the size the CLI reported after it, or none until the
//! next turn reports one. One no runner carries out any more ends as interrupted, as a turn does.

use orchester_core::{
  connection::context_use::ContextUse,
  id::ChatId,
  orchestrator::{ChatEvent, CompactionState, TurnFailure},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ProjectDb {
  /// Keeps a compaction of `chat`'s session, running, started at `at` when its context held `before` tokens; returns
  /// its entry (`seq`), by which it ends.
  pub fn begin_chat_compaction(&mut self, chat: ChatId, before: Option<u64>, at: Timestamp) -> Result<i64, StoreError> {
    let event = ChatEvent::Compaction {
      before,
      state: CompactionState::Running,
    };
    self.add_chat_event(chat, &event, at)?;
    Ok(self.connection.last_insert_rowid())
  }

  /// Ends compaction `seq` as `state`, unless it ended already; whether it changed. A compaction done leaves the
  /// conversation's session with `reading`, which is none when the CLI reported none after it.
  pub fn end_chat_compaction(
    &mut self,
    seq: i64,
    state: CompactionState,
    reading: Option<&ContextUse>,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let Some((chat, before)) = running(&transaction, seq)? else {
      return Ok(false);
    };
    let done = matches!(state, CompactionState::Done { .. });
    write(&transaction, seq, &ChatEvent::Compaction { before, state })?;
    if done {
      let used = reading.map(|reading| i64::try_from(reading.used())).transpose();
      let window = reading.and_then(ContextUse::window).map(i64::try_from).transpose();
      let (Ok(used), Ok(window)) = (used, window) else {
        return Err(corrupt("context tokens", "too many to keep"));
      };
      transaction.execute(
        "UPDATE orchestrator_chats SET context_used = ?2, context_window = ?3, context_estimated = ?4 WHERE id = ?1",
        params![chat, used, window, reading.map(ContextUse::estimated)],
      )?;
    }
    transaction.commit()?;
    Ok(true)
  }

  /// Whether a compaction of `chat`'s session is running.
  pub fn chat_compacting(&self, chat: ChatId) -> Result<bool, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT body FROM orchestrator_entries WHERE chat_id = ?1 AND kind = 'event'")?;
    let bodies: Vec<String> = statement
      .query_map([chat.to_string()], |row| row.get(0))?
      .collect::<Result<_, _>>()?;
    for body in bodies {
      if event_of(&body)?.is_some_and(|event| event.is_compacting()) {
        return Ok(true);
      }
    }
    Ok(false)
  }

  /// Ends every compaction still running as interrupted, but those of `held`, which a runner carries out, and, when
  /// `only` names a conversation, those of other conversations. Returns the conversations it changed.
  pub fn interrupt_chat_compactions(&mut self, held: &[i64], only: Option<ChatId>) -> Result<Vec<ChatId>, StoreError> {
    let transaction = self.connection.transaction()?;
    let events: Vec<(i64, String, String)> = {
      let mut statement =
        transaction.prepare("SELECT seq, chat_id, body FROM orchestrator_entries WHERE kind = 'event' ORDER BY seq")?;
      statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?
    };
    let mut chats = Vec::new();
    for (seq, chat, body) in events {
      let chat: ChatId = parse("chat id", &chat)?;
      let Some(ChatEvent::Compaction { before, .. }) = event_of(&body)?.filter(ChatEvent::is_compacting) else {
        continue;
      };
      if held.contains(&seq) || only.is_some_and(|only| only != chat) {
        continue;
      }
      let state = CompactionState::Failed {
        reason: TurnFailure::Interrupted,
      };
      write(&transaction, seq, &ChatEvent::Compaction { before, state })?;
      if !chats.contains(&chat) {
        chats.push(chat);
      }
    }
    transaction.commit()?;
    Ok(chats)
  }
}

/// The conversation and the tokens before of compaction `seq` while it runs; `None` once it ended or when it is no
/// compaction.
fn running(transaction: &Transaction<'_>, seq: i64) -> Result<Option<(String, Option<u64>)>, StoreError> {
  let row: Option<(String, String)> = transaction
    .query_row(
      "SELECT chat_id, body FROM orchestrator_entries WHERE seq = ?1 AND kind = 'event'",
      [seq],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?;
  let Some((chat, body)) = row else {
    return Ok(None);
  };
  Ok(match event_of(&body)? {
    Some(ChatEvent::Compaction {
      before,
      state: CompactionState::Running,
    }) => Some((chat, before)),
    _ => None,
  })
}

fn event_of(body: &str) -> Result<Option<ChatEvent>, StoreError> {
  serde_json::from_str(body)
    .map(Some)
    .map_err(|error| corrupt("chat event", error))
}

fn write(transaction: &Transaction<'_>, seq: i64, event: &ChatEvent) -> Result<(), StoreError> {
  let body = serde_json::to_string(event).map_err(|error| corrupt("chat event", error))?;
  transaction.execute(
    "UPDATE orchestrator_entries SET body = ?2 WHERE seq = ?1",
    params![seq, body],
  )?;
  Ok(())
}
