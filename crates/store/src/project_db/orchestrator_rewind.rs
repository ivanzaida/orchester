//! Rewinding a conversation of the Orchestrator chat (ORC-333, `orchester_core::orchestrator::rewind`): in one
//! transaction, the entries from the point on go with the attachments of their messages, the conversation forgets its
//! session and that session's context reading, and the rewound line is kept with what stays done. Nothing changes
//! while the conversation is busy, or when the point is not in it. The next turn that has no session to resume is
//! told the conversation as it was kept ([`ProjectDb::primed`]).

use std::path::PathBuf;

use orchester_core::{
  id::ChatId,
  orchestrator::{
    Chat, ChatEvent, EntryKind, RewindPoint, RewoundTo,
    rewind::{Dropped, primed_prompt, primes},
  },
  time::Timestamp,
};
use rusqlite::params;

use super::{
  ProjectDb, attachments,
  orchestrator_titles::{busy, forget_attachments},
};
use crate::StoreError;

/// What became of a person's rewind of a conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatRewind {
  /// It went back: what it dropped, and the words of the person's message it went back to before, if it went back
  /// before one.
  Rewound { dropped: Dropped, words: Option<String> },
  /// The project no longer keeps the conversation.
  Gone,
  /// Nothing changed: the Orchestrator answers in it, compacts its session, or a message waits there for its turn.
  Busy,
  /// Nothing changed: the point is not in the conversation, or nothing follows it.
  NotThere,
}

impl ProjectDb {
  /// Rewinds conversation `chat` to `point` at `at`, then removes the copies of the attachments it dropped.
  pub fn rewind_chat(&mut self, chat: ChatId, point: RewindPoint, at: Timestamp) -> Result<ChatRewind, StoreError> {
    let (rewind, copies) =
      self.with_write_transaction(|db| db.rewind_now(chat, point, at), |error| error, |_, error| error)?;
    attachments::remove_copies(&copies);
    Ok(rewind)
  }

  /// [`Self::rewind_chat`] inside its transaction: what became of it, and the copies to remove once it is committed.
  fn rewind_now(
    &mut self,
    chat: ChatId,
    point: RewindPoint,
    at: Timestamp,
  ) -> Result<(ChatRewind, Vec<PathBuf>), StoreError> {
    match busy(&self.connection, chat)? {
      None => return Ok((ChatRewind::Gone, Vec::new())),
      Some(true) => return Ok((ChatRewind::Busy, Vec::new())),
      Some(false) => {}
    }
    let entries = self.chat_entries(chat)?;
    let found = match point {
      RewindPoint::Before(message) => entries.iter().find_map(|entry| match &entry.kind {
        EntryKind::Person {
          message: kept, text, ..
        } if *kept == message => Some((
          entry.seq,
          RewoundTo::BeforeMessage { at: entry.at },
          Some(text.as_str().to_owned()),
        )),
        _ => None,
      }),
      RewindPoint::After { last, answered } => entries.iter().any(|entry| entry.seq == last).then_some((
        last + 1,
        RewoundTo::AfterAnswer { at: answered },
        None,
      )),
    };
    let Some((from, to, words)) = found.filter(|(from, ..)| entries.iter().any(|entry| entry.seq >= *from)) else {
      return Ok((ChatRewind::NotThere, Vec::new()));
    };
    let dropped = Dropped::of(&entries, from);
    let id = chat.to_string();
    let copies = forget_attachments(&self.connection, &self.directory, chat, from)?;
    self.connection.execute(
      "DELETE FROM orchestrator_entries WHERE chat_id = ?1 AND seq >= ?2",
      params![id, from],
    )?;
    // The session heard what was dropped: it is never resumed again.
    self.connection.execute(
      "UPDATE orchestrator_chats SET session = NULL, session_connection_id = NULL, session_account_id = NULL,
         context_used = NULL, context_window = NULL, context_estimated = NULL
       WHERE id = ?1",
      [&id],
    )?;
    let event = ChatEvent::Rewound {
      to,
      dropped: dropped.count(),
      done: dropped.done.clone(),
    };
    self.add_chat_event(chat, &event, at)?;
    Ok((ChatRewind::Rewound { dropped, words }, copies))
  }

  /// `prompt`, the prompt of the turn kept as entry `seq` of `chat`, as the agent gets it: after a rewind, a turn with
  /// no session to resume starts a new one, which is told the conversation kept before it first.
  pub(super) fn primed(&self, chat: &Chat, seq: i64, prompt: String) -> Result<String, StoreError> {
    if chat.session().is_some() {
      return Ok(prompt);
    }
    let mut kept = self.chat_entries(chat.id())?;
    kept.retain(|entry| entry.seq < seq);
    Ok(if primes(&kept) {
      primed_prompt(&kept, &prompt)
    } else {
      prompt
    })
  }
}
