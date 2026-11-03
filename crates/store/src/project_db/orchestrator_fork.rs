//! Forking a conversation of the Orchestrator chat (ORC-334, `orchester_core::orchestrator::fork`): in one
//! transaction, a new conversation is kept with its line, copies of the original's entries up to the point, and what
//! the original did after it; the original is only read. Each copy keeps its kind, time, body and how its turn went,
//! under message ids of its own (the person's messages and the turns of waits and reports), with its turn's todo list.
//! A turn still under way when the fork is made is copied as stopped there; a message still waiting for its turn is
//! not copied. A copy of a wait or a daily report is kept by a key of its own and never starts a turn by itself. The
//! attachments of the copied messages are shared, not written again ([`attachments::share`]). The fork has no session:
//! its first turn is told the copied conversation ([`ProjectDb::primed`]).

use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::{
    ChatEntry, ChatEvent, Dropped, EntryKind, RewindPoint,
    fork::{cut, takes},
  },
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{
  ProjectDb,
  attachments::{self, AttachmentOwner, Written},
};
use crate::StoreError;

/// What became of a fork of a conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatFork {
  /// The new conversation is kept, and the words of the person's message it stopped before, if it stopped before one.
  Forked { chat: ChatId, words: Option<String> },
  /// The project no longer keeps the conversation.
  Gone,
  /// Nothing was kept: the point is not in the conversation.
  NotThere,
}

impl ProjectDb {
  /// Forks conversation `from` at `point` into the new conversation `chat`, titled `title`, at `at`.
  pub fn fork_chat(
    &mut self,
    (from, point): (ChatId, RewindPoint),
    (chat, title): (ChatId, &str),
    at: Timestamp,
  ) -> Result<ChatFork, StoreError> {
    let (fork, copies) = self.with_write_transaction(
      |db| db.fork_now((from, point), (chat, title), at),
      |error| error,
      |_, error| error,
    )?;
    copies.into_iter().for_each(Written::keep);
    Ok(fork)
  }

  /// [`Self::fork_chat`] inside its transaction: what became of it, and the shared copies of attachments it wrote,
  /// which are removed again unless it is committed.
  fn fork_now(
    &mut self,
    (from, point): (ChatId, RewindPoint),
    (chat, title): (ChatId, &str),
    at: Timestamp,
  ) -> Result<(ChatFork, Vec<Written>), StoreError> {
    let Some(original) = self.chat(from)? else {
      return Ok((ChatFork::Gone, Vec::new()));
    };
    let entries = self.chat_entries(from)?;
    let Some(end) = cut(&entries, point) else {
      return Ok((ChatFork::NotThere, Vec::new()));
    };
    let (point_at, words) = match point {
      RewindPoint::Before(_) => entries
        .iter()
        .find(|entry| entry.seq == end)
        .map(|entry| match &entry.kind {
          EntryKind::Person { text, .. } => (entry.at, Some(text.as_str().to_owned())),
          _ => (entry.at, None),
        })
        .ok_or_else(|| crate::rows::corrupt("chat fork", "its point is gone"))?,
      RewindPoint::After { answered, .. } => (answered, None),
    };
    self.connection.execute(
      "INSERT INTO orchestrator_chats (id, title, started_at, plan_only) VALUES (?1, ?2, ?3, ?4)",
      params![chat.to_string(), title, at.unix_millis(), original.plan_only()],
    )?;
    let forked = ChatEvent::Forked {
      from,
      title: original.title().to_owned(),
      at: point_at,
    };
    self.add_chat_event(chat, &forked, at)?;
    let mut copies = Vec::new();
    for entry in entries.iter().filter(|entry| entry.seq < end && takes(&entry.kind)) {
      copies.extend(self.copy_entry(chat, entry, at)?);
    }
    let made = Dropped::of(&entries, end).done;
    if !made.is_empty() {
      self.add_chat_event(chat, &ChatEvent::MadeInOriginal { made }, at)?;
    }
    Ok((ChatFork::Forked { chat, words }, copies))
  }

  /// Keeps a copy of `entry` in `chat`, a turn under way stopped at `at`, with its turn's todo list and, for a
  /// person's message, its attachments shared; the copies of those, unless it has none.
  fn copy_entry(&mut self, chat: ChatId, entry: &ChatEntry, at: Timestamp) -> Result<Option<Written>, StoreError> {
    let old: Option<String> = self
      .connection
      .query_row(
        "SELECT message_id FROM orchestrator_entries WHERE seq = ?1",
        [entry.seq],
        |row| row.get(0),
      )
      .optional()?
      .flatten();
    let message = ChatMessageId::generate();
    self.connection.execute(
      "INSERT INTO orchestrator_entries
         (chat_id, kind, at, body, message_id, turn, failure, model, ended_at, by_automation, mode, wait_key)
       SELECT ?1, kind, at,
         CASE WHEN kind IN ('wait', 'report') THEN json_set(body, '$.automatic', json('false')) ELSE body END,
         CASE WHEN message_id IS NULL THEN NULL ELSE ?2 END,
         CASE WHEN turn = 'thinking' THEN 'stopped' ELSE turn END,
         failure, model,
         CASE WHEN turn = 'thinking' THEN ?4 ELSE ended_at END,
         by_automation, mode,
         CASE WHEN wait_key IS NULL THEN NULL ELSE 'fork:' || ?1 || ':' || wait_key END
       FROM orchestrator_entries WHERE seq = ?3",
      params![chat.to_string(), message.to_string(), entry.seq, at.unix_millis()],
    )?;
    let Some(old) = old else {
      return Ok(None);
    };
    self.connection.execute(
      "INSERT INTO orchestrator_todos (message_id, items, updated_at)
       SELECT ?2, items, updated_at FROM orchestrator_todos WHERE message_id = ?1",
      params![old, message.to_string()],
    )?;
    let EntryKind::Person { message: original, .. } = &entry.kind else {
      return Ok(None);
    };
    let kept = self.attachments_of(AttachmentOwner::ChatMessage(*original))?;
    if kept.is_empty() {
      return Ok(None);
    }
    let (written, shared) = attachments::share(&self.directory, &kept)?;
    attachments::insert(&self.connection, AttachmentOwner::ChatMessage(message), &shared, at)?;
    Ok(Some(written))
  }
}
