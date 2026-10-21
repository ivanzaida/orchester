//! The project's Orchestrator chat (`orchester_core::orchestrator`): what it runs on, its conversations and their
//! entries. The app keeps a person's message before it asks the runner for the turn, and forgets it again when the
//! runner never took it; the runner keeps the replies, the session and how the turn ended; the app keeps the
//! actions the agent took through its tools, and the waits of the project's agents with the turns they started
//! ([`super::orchestrator_waits`]).

use orchester_core::{
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId, ConnectionId},
  orchestrator::{
    AgentWait, Chat, ChatAction, ChatEntry, ChatEvent, ChatSession, EntryKind, OrchestratorSettings, TurnFailure,
    TurnState, WaitTurn,
    instructions::{report_prompt, wait_prompt},
  },
  resource::ModelId,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};

use super::{ProjectDb, orchestrator_reports::report_of};
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
  /// What the Orchestrator runs on: its latest published version; `None` until a person published one.
  pub fn orchestrator_settings(&self) -> Result<Option<OrchestratorSettings>, StoreError> {
    Ok(self.orchestrator_published()?.map(|(_, settings)| settings))
  }

  /// The latest published version of what the Orchestrator runs on, with its number.
  pub fn orchestrator_published(&self) -> Result<Option<(u32, OrchestratorSettings)>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT version, connection_id, model, effort FROM orchestrator_versions ORDER BY version DESC LIMIT 1",
        [],
        |row| Ok((row.get::<_, i64>(0)?, settings_row(row, 1)?)),
      )
      .optional()?;
    row
      .map(|(version, raw)| {
        let version = u32::try_from(version).map_err(|error| corrupt("orchestrator version", error))?;
        Ok((version, raw.into_settings()?))
      })
      .transpose()
  }

  /// The draft saved since the latest published version; `None` when there is none.
  pub fn orchestrator_draft(&self) -> Result<Option<OrchestratorSettings>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT connection_id, model, effort FROM orchestrator_draft WHERE id = 1",
        [],
        |row| settings_row(row, 0),
      )
      .optional()?
      .map(RawSettings::into_settings)
      .transpose()
  }

  /// Saves `settings` as the draft, at `at`, in place of the one saved before.
  pub fn save_orchestrator_draft(&mut self, settings: &OrchestratorSettings, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO orchestrator_draft (id, connection_id, model, effort, saved_at) VALUES (1, ?1, ?2, ?3, ?4)
       ON CONFLICT (id) DO UPDATE SET connection_id = excluded.connection_id, model = excluded.model,
         effort = excluded.effort, saved_at = excluded.saved_at",
      params![
        settings.connection().to_string(),
        settings.model().as_str(),
        settings.effort(),
        at.unix_millis()
      ],
    )?;
    Ok(())
  }

  /// Publishes `settings` at `at` as the next version, which the chat answers on from its next reply; the draft is
  /// done with. Returns the version's number.
  pub fn publish_orchestrator(&mut self, settings: &OrchestratorSettings, at: Timestamp) -> Result<u32, StoreError> {
    let transaction = self.connection.transaction()?;
    let latest: i64 = transaction.query_row(
      "SELECT COALESCE(MAX(version), 0) FROM orchestrator_versions",
      [],
      |row| row.get(0),
    )?;
    let version = latest + 1;
    transaction.execute(
      "INSERT INTO orchestrator_versions (version, connection_id, model, effort, published_at)
       VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        version,
        settings.connection().to_string(),
        settings.model().as_str(),
        settings.effort(),
        at.unix_millis()
      ],
    )?;
    transaction.execute("DELETE FROM orchestrator_draft", [])?;
    transaction.commit()?;
    u32::try_from(version).map_err(|error| corrupt("orchestrator version", error))
  }

  /// Keeps the person's message `message` of `chat` (stored with it when it is new), with its turn thinking.
  /// Returns whether it was new: a message kept before is never kept twice, whatever its turn did since.
  pub fn keep_chat_message(
    &mut self,
    chat: &Chat,
    (message, text): (ChatMessageId, &MessageBody),
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT OR IGNORE INTO orchestrator_chats (id, title, started_at) VALUES (?1, ?2, ?3)",
      params![chat.id().to_string(), chat.title(), chat.started().unix_millis()],
    )?;
    let kept = transaction.execute(
      "INSERT OR IGNORE INTO orchestrator_entries (chat_id, kind, at, body, message_id, turn)
       VALUES (?1, 'person', ?2, ?3, ?4, 'thinking')",
      params![
        chat.id().to_string(),
        at.unix_millis(),
        text.as_str(),
        message.to_string()
      ],
    )?;
    transaction.commit()?;
    Ok(kept == 1)
  }

  /// Forgets the person's message `message`, which the runner never took: its draft is kept by the app, and a
  /// conversation it alone started goes with it.
  pub fn forget_chat_message(&mut self, message: ChatMessageId) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let chat: Option<String> = transaction
      .query_row(
        "SELECT chat_id FROM orchestrator_entries WHERE message_id = ?1 AND turn = 'thinking'",
        [message.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    if let Some(chat) = chat {
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
    Ok(())
  }

  /// The turn of `message`: its prompt, its conversation and where it stands; `None` when it is not kept.
  pub fn chat_turn(&self, message: ChatMessageId) -> Result<Option<ChatTurn>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT chat_id, kind, body, turn, failure FROM orchestrator_entries WHERE message_id = ?1",
        [message.to_string()],
        |row| {
          Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
          ))
        },
      )
      .optional()?;
    let Some((chat, kind, body, turn, failure)) = row else {
      return Ok(None);
    };
    let chat = self
      .chat(parse("chat id", &chat)?)?
      .ok_or_else(|| corrupt("chat message", "its conversation is gone"))?;
    let prompt = match kind.as_str() {
      "wait" => wait_prompt(&wait_of(&body)?),
      "report" => report_prompt(&report_of(&body)?),
      _ => body,
    };
    Ok(Some(ChatTurn {
      chat,
      text: MessageBody::new(&prompt).map_err(|error| corrupt("chat message", error))?,
      turn: turn_state(&turn, failure.as_deref())?,
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
  /// names a conversation, those of other conversations; returns the conversations it changed.
  pub fn interrupt_chat_turns(
    &mut self,
    held: &[ChatMessageId],
    only: Option<ChatId>,
  ) -> Result<Vec<ChatId>, StoreError> {
    let failure = failure_column(&TurnState::Failed(TurnFailure::Interrupted))?;
    let transaction = self.connection.transaction()?;
    let thinking = {
      let mut statement =
        transaction.prepare("SELECT message_id, chat_id FROM orchestrator_entries WHERE turn = 'thinking'")?;
      statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .map(|row| {
          let (message, chat) = row?;
          Ok((parse("chat message id", &message)?, parse("chat id", &chat)?))
        })
        .collect::<Result<Vec<(ChatMessageId, ChatId)>, StoreError>>()?
    };
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
    transaction.commit()?;
    Ok(chats)
  }

  /// Sets the CLI session `chat`'s turns continue, on `connection` and its `account`.
  pub fn set_chat_session(
    &mut self,
    chat: ChatId,
    session: &str,
    (connection, account): (ConnectionId, AgentAccountId),
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE orchestrator_chats SET session = ?2, session_connection_id = ?3, session_account_id = ?4 WHERE id = ?1",
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

  /// The conversation `chat`; `None` when it is not kept.
  pub fn chat(&self, chat: ChatId) -> Result<Option<Chat>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT id, title, started_at, session, session_connection_id, session_account_id FROM orchestrator_chats
         WHERE id = ?1",
        [chat.to_string()],
        raw_chat,
      )
      .optional()?
      .map(RawChat::into_chat)
      .transpose()
  }

  /// Every conversation, the one with the latest entry first.
  pub fn chats(&self) -> Result<Vec<ChatListing>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT c.id, c.title, c.started_at, c.session, c.session_connection_id, c.session_account_id,
         COALESCE(MAX(e.at), c.started_at), COUNT(e.seq) FILTER (WHERE e.kind IN ('person', 'reply'))
       FROM orchestrator_chats c LEFT JOIN orchestrator_entries e ON e.chat_id = c.id
       GROUP BY c.id ORDER BY 7 DESC, c.id DESC",
    )?;
    let rows = statement.query_map([], |row| {
      Ok((raw_chat(row)?, row.get::<_, i64>(6)?, row.get::<_, i64>(7)?))
    })?;
    rows
      .map(|row| {
        let (chat, last, messages) = row?;
        Ok(ChatListing {
          chat: chat.into_chat()?,
          last: Timestamp::from_unix_millis(last),
          messages: u32::try_from(messages).map_err(|error| corrupt("chat message count", error))?,
        })
      })
      .collect()
  }

  /// The entries of `chat` in the order they were kept.
  pub fn chat_entries(&self, chat: ChatId) -> Result<Vec<ChatEntry>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT seq, kind, at, body, message_id, turn, failure, model, ended_at FROM orchestrator_entries
       WHERE chat_id = ?1 ORDER BY seq",
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
      })
    })?;
    rows.map(|row| row?.into_entry()).collect()
  }
}

/// What the Orchestrator runs on, as a row holds it from column `first` on.
struct RawSettings {
  connection: String,
  model: String,
  effort: Option<String>,
}

fn settings_row(row: &Row<'_>, first: usize) -> rusqlite::Result<RawSettings> {
  Ok(RawSettings {
    connection: row.get(first)?,
    model: row.get(first + 1)?,
    effort: row.get(first + 2)?,
  })
}

impl RawSettings {
  fn into_settings(self) -> Result<OrchestratorSettings, StoreError> {
    let model = ModelId::new(&self.model).map_err(|error| corrupt("orchestrator model", error))?;
    Ok(OrchestratorSettings::new(
      parse("orchestrator connection", &self.connection)?,
      model,
      self.effort,
    ))
  }
}

struct RawChat {
  id: String,
  title: String,
  started: i64,
  session: Option<String>,
  connection: Option<String>,
  account: Option<String>,
}

fn raw_chat(row: &Row<'_>) -> rusqlite::Result<RawChat> {
  Ok(RawChat {
    id: row.get(0)?,
    title: row.get(1)?,
    started: row.get(2)?,
    session: row.get(3)?,
    connection: row.get(4)?,
    account: row.get(5)?,
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
    Ok(Chat::restore(
      parse("chat id", &self.id)?,
      self.title,
      Timestamp::from_unix_millis(self.started),
      session,
    ))
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
}

impl RawEntry {
  fn into_entry(self) -> Result<ChatEntry, StoreError> {
    let kind = match (self.kind.as_str(), self.message, self.turn, self.model) {
      ("person", Some(message), Some(turn), _) => EntryKind::Person {
        message: parse("chat message id", &message)?,
        text: MessageBody::new(&self.body).map_err(|error| corrupt("chat message", error))?,
        turn: turn_state(&turn, self.failure.as_deref())?,
        ended: self.ended.map(Timestamp::from_unix_millis),
      },
      ("reply", _, _, Some(model)) => EntryKind::Reply {
        text: self.body,
        model: ModelId::new(&model).map_err(|error| corrupt("chat reply model", error))?,
      },
      ("action", ..) => {
        EntryKind::Action(serde_json::from_str(&self.body).map_err(|error| corrupt("chat action", error))?)
      }
      ("event", ..) => {
        EntryKind::Event(serde_json::from_str(&self.body).map_err(|error| corrupt("chat event", error))?)
      }
      ("wait", message, turn, _) => EntryKind::Wait {
        turn: started_turn((message, turn), self.failure.as_deref(), self.ended)?,
        wait: wait_of(&self.body)?,
      },
      ("report", message, turn, _) => EntryKind::Report {
        turn: started_turn((message, turn), self.failure.as_deref(), self.ended)?,
        report: Box::new(report_of(&self.body)?),
      },
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
    (word, _) => return Err(corrupt("chat turn", word)),
  })
}

fn failure_column(state: &TurnState) -> Result<Option<String>, StoreError> {
  match state {
    TurnState::Failed(failure) => serde_json::to_string(failure)
      .map(Some)
      .map_err(|error| corrupt("chat turn failure", error)),
    _ => Ok(None),
  }
}
