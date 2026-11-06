//! The project's Orchestrator chat (`orchester_core::orchestrator`): its conversations and their entries (what it
//! runs on is [`super::orchestrator_settings`]'s). The app keeps a person's message before it asks the runner for the
//! turn, and forgets it again when the runner never took it; the runner keeps the replies, the session and how the
//! turn ended; the app keeps the actions the agent took through its tools, the plans it answered with in plan-only
//! mode ([`super::orchestrator_plans`]), and the waits of the project's agents with the turns they started
//! ([`super::orchestrator_waits`]).

use orchester_core::{
  attachment::NewAttachment,
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId},
  orchestrator::{
    AgentWait, Chat, ChatAction, ChatEntry, ChatEvent, ChatSession, EntryKind, Queued, TurnFailure, TurnMode,
    TurnState, WaitTurn,
    instructions::{report_prompt, wait_prompt, watch_summary_prompt},
    plan::plan_prompt,
  },
  resource::ModelId,
  run::Answerer,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};

use super::{
  ProjectDb,
  attachments::{self, AttachmentOwner},
  orchestrator_plans::plan_of,
  orchestrator_reports::report_of,
  orchestrator_watch_summaries::summary_of,
};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// A conversation as History lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatListing {
  pub chat: Chat,
  /// When its last entry was kept.
  pub last: Timestamp,
  /// The person's messages and the agent's replies in it.
  pub messages: u32,
  /// When a person pinned it to History's top; `None` while it is not pinned.
  pub pinned: Option<Timestamp>,
  /// When a person archived it, which lists it under History's Archived; `None` while it is not archived.
  pub archived: Option<Timestamp>,
}

/// A turn's prompt, as the runner reads it to start the turn: a person's message, or what a wait of an agent that
/// started the turn by itself tells the Orchestrator ([`wait_prompt`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatTurn {
  pub chat: Chat,
  pub text: MessageBody,
  pub turn: TurnState,
}

impl ProjectDb {
  /// Keeps the person's message `message` of `chat` (stored with it when it is new), with its turn thinking.
  /// Returns whether it was new: a message kept before is never kept twice, whatever its turn did since.
  pub fn keep_chat_message(
    &mut self,
    chat: &Chat,
    message: (ChatMessageId, &MessageBody),
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    self.keep_chat_message_with(chat, message, (&[], Answerer::Person), at)
  }

  /// [`Self::keep_chat_message`] with `attachments`, whose copies are written before the rows and removed again when
  /// the message is not kept now (a message kept before keeps the attachments it was kept with), sent `by` the person
  /// or by automation driving the window.
  pub fn keep_chat_message_with(
    &mut self,
    chat: &Chat,
    message: (ChatMessageId, &MessageBody),
    sent: (&[NewAttachment], Answerer),
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    self.keep_chat_message_as(chat, message, sent, (&TurnState::Thinking, at))
  }

  /// Forgets the person's message `message`, which the runner never took, with its attachments: its draft is kept
  /// by the app, and a conversation it alone started goes with it.
  pub fn forget_chat_message(&mut self, message: ChatMessageId) -> Result<(), StoreError> {
    self.forget_person(message, &["thinking"]).map(drop)
  }

  /// Forgets the person's message `message` with its attachments while its turn is one of `turns`, and the
  /// conversation it alone started; whether it was forgotten.
  pub(super) fn forget_person(&mut self, message: ChatMessageId, turns: &[&str]) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let kept: Option<(String, String)> = transaction
      .query_row(
        "SELECT chat_id, turn FROM orchestrator_entries WHERE message_id = ?1 AND kind = 'person'",
        [message.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    let chat = kept
      .filter(|(_, turn)| turns.contains(&turn.as_str()))
      .map(|(chat, _)| chat);
    let forgotten = chat.is_some();
    let mut copies = Vec::new();
    if let Some(chat) = chat {
      copies = attachments::forget(&transaction, &self.directory, AttachmentOwner::ChatMessage(message))?;
      transaction.execute(
        "DELETE FROM orchestrator_entries WHERE message_id = ?1",
        [message.to_string()],
      )?;
      transaction.execute(
        "DELETE FROM orchestrator_chats WHERE id = ?1
           AND NOT EXISTS (SELECT 1 FROM orchestrator_entries WHERE chat_id = ?1)",
        [&chat],
      )?;
    }
    transaction.commit()?;
    attachments::remove_copies(&copies);
    Ok(forgotten)
  }

  /// The turn of `message`: its prompt, its conversation and where it stands; `None` when it is not kept. A turn that
  /// plans tells the agent so before the person's words ([`plan_prompt`]); the first turn after a rewind is told the
  /// conversation as it was kept ([`Self::primed`]).
  pub fn chat_turn(&self, message: ChatMessageId) -> Result<Option<ChatTurn>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT chat_id, kind, body, turn, failure, mode, seq FROM orchestrator_entries WHERE message_id = ?1",
        [message.to_string()],
        |row| {
          Ok((
            [
              row.get::<_, String>(0)?,
              row.get::<_, String>(1)?,
              row.get::<_, String>(2)?,
            ],
            row.get::<_, String>(3)?,
            [row.get::<_, Option<String>>(4)?, row.get::<_, Option<String>>(5)?],
            row.get::<_, i64>(6)?,
          ))
        },
      )
      .optional()?;
    let Some(([chat, kind, body], turn, [failure, mode], seq)) = row else {
      return Ok(None);
    };
    let chat = self
      .chat(parse("chat id", &chat)?)?
      .ok_or_else(|| corrupt("chat message", "its conversation is gone"))?;
    let mode = TurnMode::of_word(mode.as_deref()).map_err(|error| corrupt("chat turn mode", error))?;
    let prompt = match (kind.as_str(), mode) {
      ("wait", _) => wait_prompt(&wait_of(&body)?),
      ("report", _) => match summary_of(&body)? {
        Some(summary) => watch_summary_prompt(&summary),
        None => report_prompt(&report_of(&body)?),
      },
      (_, TurnMode::Plans) => plan_prompt(&body),
      _ => body,
    };
    let turn = turn_state(&turn, failure.as_deref())?;
    // A message handed to a turn under way is not a turn's prompt of its own.
    let prompt = if turn.has_turn() {
      self.primed(&chat, seq, prompt)?
    } else {
      prompt
    };
    Ok(Some(ChatTurn {
      chat,
      text: MessageBody::new(&prompt).map_err(|error| corrupt("chat message", error))?,
      turn,
    }))
  }

  /// Ends the turn of `message` as `state` at `at`, unless it ended already. Returns whether it changed.
  pub fn end_chat_turn(
    &mut self,
    message: ChatMessageId,
    state: &TurnState,
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE orchestrator_entries SET turn = ?2, failure = ?3, ended_at = ?4 WHERE message_id = ?1 AND turn = 'thinking'",
      params![message.to_string(), state.word(), failure_column(state)?, at.unix_millis()],
    )?;
    Ok(changed == 1)
  }

  /// Ends every turn still thinking as interrupted, but those of `held`, which a runner answers, and, when `only`
  /// names a conversation, those of other conversations; a message handed to a turn no runner answers is left for a
  /// turn of its own ([`TurnState::Queued`]). Returns the conversations it changed.
  pub fn interrupt_chat_turns(
    &mut self,
    held: &[ChatMessageId],
    only: Option<ChatId>,
  ) -> Result<Vec<ChatId>, StoreError> {
    let failure = failure_column(&TurnState::Failed(TurnFailure::Interrupted))?;
    let transaction = self.connection.transaction()?;
    let thinking = entries_in(&transaction, "thinking")?;
    let mut chats = Vec::new();
    for (message, chat) in thinking {
      if held.contains(&message) || only.is_some_and(|only| only != chat) {
        continue;
      }
      transaction.execute(
        "UPDATE orchestrator_entries SET turn = 'failed', failure = ?2 WHERE message_id = ?1 AND turn = 'thinking'",
        params![message.to_string(), failure],
      )?;
      if !chats.contains(&chat) {
        chats.push(chat);
      }
    }
    for chat in super::orchestrator_queue::release_unheld_steering(&transaction, held, only)? {
      if !chats.contains(&chat) {
        chats.push(chat);
      }
    }
    transaction.commit()?;
    Ok(chats)
  }

  /// Sets the CLI session `chat`'s turns continue, on `connection` and its `account`. Another session than the one
  /// before starts with an empty context: the reading of the one before is forgotten, and the window its CLI reported.
  pub fn set_chat_session(
    &mut self,
    chat: ChatId,
    session: &str,
    (connection, account): (ConnectionId, AgentAccountId),
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE orchestrator_chats SET session = ?2, session_connection_id = ?3, session_account_id = ?4,
         context_used = CASE WHEN session IS ?2 THEN context_used END,
         context_window = CASE WHEN session IS ?2 THEN context_window END,
         context_estimated = CASE WHEN session IS ?2 THEN context_estimated END,
         reported_window = CASE WHEN session IS ?2 THEN reported_window END,
         reported_window_model = CASE WHEN session IS ?2 THEN reported_window_model END
       WHERE id = ?1",
      params![chat.to_string(), session, connection.to_string(), account.to_string()],
    )?;
    Ok(())
  }

  /// Keeps a reply of the agent, written by `model`.
  pub fn add_chat_reply(
    &mut self,
    chat: ChatId,
    (text, model): (&str, &ModelId),
    at: Timestamp,
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO orchestrator_entries (chat_id, kind, at, body, model) VALUES (?1, 'reply', ?2, ?3, ?4)",
      params![chat.to_string(), at.unix_millis(), text, model.as_str()],
    )?;
    Ok(())
  }

  /// Keeps an event of the conversation.
  pub fn add_chat_event(&mut self, chat: ChatId, event: &ChatEvent, at: Timestamp) -> Result<(), StoreError> {
    let body = serde_json::to_string(event).map_err(|error| corrupt("chat event", error))?;
    self.connection.execute(
      "INSERT INTO orchestrator_entries (chat_id, kind, at, body) VALUES (?1, 'event', ?2, ?3)",
      params![chat.to_string(), at.unix_millis(), body],
    )?;
    Ok(())
  }

  /// Keeps an action the agent took through a tool.
  pub fn add_chat_action(&mut self, chat: ChatId, action: &ChatAction, at: Timestamp) -> Result<(), StoreError> {
    let body = serde_json::to_string(action).map_err(|error| corrupt("chat action", error))?;
    self.connection.execute(
      "INSERT INTO orchestrator_entries (chat_id, kind, at, body) VALUES (?1, 'action', ?2, ?3)",
      params![chat.to_string(), at.unix_millis(), body],
    )?;
    Ok(())
  }

  /// The conversation `chat`, titled as a person renamed it if they did; `None` when it is not kept.
  pub fn chat(&self, chat: ChatId) -> Result<Option<Chat>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT id, COALESCE(person_title, title), started_at, session, session_connection_id, session_account_id,
           plan_only
         FROM orchestrator_chats WHERE id = ?1",
        [chat.to_string()],
        |row| raw_chat(row, 6),
      )
      .optional()?
      .map(RawChat::into_chat)
      .transpose()
  }

  /// Every conversation, archived ones too, titled as [`Self::chat`] titles it, the one with the latest entry first.
  pub fn chats(&self) -> Result<Vec<ChatListing>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT c.id, COALESCE(c.person_title, c.title), c.started_at, c.session, c.session_connection_id,
         c.session_account_id, COALESCE(MAX(e.at), c.started_at),
         COUNT(e.seq) FILTER (WHERE e.kind IN ('person', 'reply')), c.pinned_at, c.archived_at, c.plan_only
       FROM orchestrator_chats c LEFT JOIN orchestrator_entries e ON e.chat_id = c.id
       GROUP BY c.id ORDER BY 7 DESC, c.id DESC",
    )?;
    let rows = statement.query_map([], |row| {
      Ok((
        raw_chat(row, 10)?,
        row.get::<_, i64>(6)?,
        row.get::<_, i64>(7)?,
        [row.get::<_, Option<i64>>(8)?, row.get::<_, Option<i64>>(9)?],
      ))
    })?;
    rows
      .map(|row| {
        let (chat, last, messages, [pinned, archived]) = row?;
        Ok(ChatListing {
          chat: chat.into_chat()?,
          last: Timestamp::from_unix_millis(last),
          messages: u32::try_from(messages).map_err(|error| corrupt("chat message count", error))?,
          pinned: pinned.map(Timestamp::from_unix_millis),
          archived: archived.map(Timestamp::from_unix_millis),
        })
      })
      .collect()
  }

  /// The entries of `chat` in the order they were kept.
  pub fn chat_entries(&self, chat: ChatId) -> Result<Vec<ChatEntry>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT seq, kind, at, body, message_id, turn, failure, model, ended_at, by_automation, mode
       FROM orchestrator_entries WHERE chat_id = ?1 ORDER BY seq",
    )?;
    let rows = statement.query_map([chat.to_string()], |row| {
      Ok(RawEntry {
        seq: row.get(0)?,
        kind: row.get(1)?,
        at: row.get(2)?,
        body: row.get(3)?,
        message: row.get(4)?,
        turn: row.get(5)?,
        failure: row.get(6)?,
        model: row.get(7)?,
        ended: row.get(8)?,
        by_automation: row.get(9)?,
        mode: row.get(10)?,
      })
    })?;
    rows.map(|row| row?.into_entry()).collect()
  }
}

struct RawChat {
  id: String,
  title: String,
  started: i64,
  session: Option<String>,
  connection: Option<String>,
  account: Option<String>,
  plan_only: bool,
}

/// A conversation as its row holds it, its *Plan only* switch in column `plan_only`.
fn raw_chat(row: &Row<'_>, plan_only: usize) -> rusqlite::Result<RawChat> {
  Ok(RawChat {
    id: row.get(0)?,
    title: row.get(1)?,
    started: row.get(2)?,
    session: row.get(3)?,
    connection: row.get(4)?,
    account: row.get(5)?,
    plan_only: row.get(plan_only)?,
  })
}

impl RawChat {
  fn into_chat(self) -> Result<Chat, StoreError> {
    let session = match (self.session, self.connection, self.account) {
      (Some(id), Some(connection), Some(account)) => Some(ChatSession {
        id,
        connection: parse("chat session connection", &connection)?,
        account: parse("chat session account", &account)?,
      }),
      _ => None,
    };
    Ok(
      Chat::restore(
        parse("chat id", &self.id)?,
        self.title,
        Timestamp::from_unix_millis(self.started),
        session,
      )
      .with_plan_only(self.plan_only),
    )
  }
}

struct RawEntry {
  seq: i64,
  kind: String,
  at: i64,
  body: String,
  message: Option<String>,
  turn: Option<String>,
  failure: Option<String>,
  model: Option<String>,
  ended: Option<i64>,
  by_automation: bool,
  mode: Option<String>,
}

impl RawEntry {
  fn into_entry(self) -> Result<ChatEntry, StoreError> {
    let kind = match (self.kind.as_str(), self.message, self.turn, self.model) {
      ("person", Some(message), Some(turn), _) => EntryKind::Person {
        message: parse("chat message id", &message)?,
        text: MessageBody::new(&self.body).map_err(|error| corrupt("chat message", error))?,
        turn: turn_state(&turn, self.failure.as_deref())?,
        ended: self.ended.map(Timestamp::from_unix_millis),
        sent_by: if self.by_automation {
          Answerer::Automation
        } else {
          Answerer::Person
        },
        mode: TurnMode::of_word(self.mode.as_deref()).map_err(|error| corrupt("chat turn mode", error))?,
      },
      ("reply", _, _, Some(model)) => EntryKind::Reply {
        text: self.body,
        model: ModelId::new(&model).map_err(|error| corrupt("chat reply model", error))?,
      },
      ("action", ..) => {
        EntryKind::Action(serde_json::from_str(&self.body).map_err(|error| corrupt("chat action", error))?)
      }
      ("event", ..) => match plan_of(&self.body)? {
        Some(plan) => EntryKind::Plan(Box::new(plan)),
        None => EntryKind::Event(serde_json::from_str(&self.body).map_err(|error| corrupt("chat event", error))?),
      },
      ("wait", message, turn, _) => EntryKind::Wait {
        turn: started_turn((message, turn), self.failure.as_deref(), self.ended)?,
        wait: wait_of(&self.body)?,
      },
      ("report", message, turn, _) => {
        let turn = started_turn((message, turn), self.failure.as_deref(), self.ended)?;
        match summary_of(&self.body)? {
          Some(summary) => EntryKind::WatchEnded {
            summary: Box::new(summary),
            turn,
          },
          None => EntryKind::Report {
            turn,
            report: Box::new(report_of(&self.body)?),
          },
        }
      }
      (kind, ..) => return Err(corrupt("chat entry kind", kind)),
    };
    Ok(ChatEntry {
      seq: self.seq,
      at: Timestamp::from_unix_millis(self.at),
      kind,
    })
  }
}

/// The turn a wait or a report started by itself, from its entry's message and turn columns; `None` while it started
/// none.
fn started_turn(
  (message, turn): (Option<String>, Option<String>),
  failure: Option<&str>,
  ended: Option<i64>,
) -> Result<Option<WaitTurn>, StoreError> {
  let (Some(message), Some(turn)) = (message, turn) else {
    return Ok(None);
  };
  Ok(Some(WaitTurn {
    message: parse("chat message id", &message)?,
    state: turn_state(&turn, failure)?,
    ended: ended.map(Timestamp::from_unix_millis),
  }))
}

/// The wait a `wait` entry's body holds.
pub(super) fn wait_of(body: &str) -> Result<AgentWait, StoreError> {
  serde_json::from_str(body).map_err(|error| corrupt("chat wait", error))
}

fn turn_state(word: &str, failure: Option<&str>) -> Result<TurnState, StoreError> {
  Ok(match (word, failure) {
    ("thinking", _) => TurnState::Thinking,
    ("answered", _) => TurnState::Answered,
    ("stopped", _) => TurnState::Stopped,
    ("failed", Some(failure)) => {
      TurnState::Failed(serde_json::from_str(failure).map_err(|error| corrupt("chat turn failure", error))?)
    }
    ("queued", _) => TurnState::Queued(Queued::AfterAnswer),
    ("queued_next", _) => TurnState::Queued(Queued::Next),
    ("not_taken", _) => TurnState::Queued(Queued::NotTaken),
    ("steering", _) => TurnState::Steering,
    ("joined", _) => TurnState::Joined,
    (word, _) => return Err(corrupt("chat turn", word)),
  })
}

/// The messages and conversations of the entries whose turn is `word`, in the order they were kept.
pub(super) fn entries_in(
  connection: &rusqlite::Connection,
  word: &str,
) -> Result<Vec<(ChatMessageId, ChatId)>, StoreError> {
  let mut statement =
    connection.prepare("SELECT message_id, chat_id FROM orchestrator_entries WHERE turn = ?1 ORDER BY seq")?;
  statement
    .query_map([word], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
    .map(|row| {
      let (message, chat) = row?;
      Ok((parse("chat message id", &message)?, parse("chat id", &chat)?))
    })
    .collect()
}

pub(super) fn failure_column(state: &TurnState) -> Result<Option<String>, StoreError> {
  match state {
    TurnState::Failed(failure) => serde_json::to_string(failure)
      .map(Some)
      .map_err(|error| corrupt("chat turn failure", error)),
    _ => Ok(None),
  }
}
