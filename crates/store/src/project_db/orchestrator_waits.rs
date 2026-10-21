//! The waits of the project's agents in the Orchestrator chat (`orchester_core::orchestrator::wait`): each kept once,
//! by its key, in the conversation the app chose, and the turn a wait starts by itself, which only starts while no
//! turn of its conversation thinks.

use std::collections::HashSet;

use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::{AgentWait, Chat},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, orchestrator::wait_of};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// A wait kept without a turn, which starts one by itself, and the conversation it was kept in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptWait {
  pub chat: ChatId,
  pub seq: i64,
  pub key: String,
  pub wait: AgentWait,
}

impl ProjectDb {
  /// The keys of every wait kept, in any conversation.
  pub fn kept_wait_keys(&self) -> Result<HashSet<String>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT wait_key FROM orchestrator_entries WHERE kind = 'wait'")?;
    let keys = statement
      .query_map([], |row| row.get::<_, String>(0))?
      .collect::<Result<HashSet<_>, _>>()?;
    Ok(keys)
  }

  /// Keeps `wait`, known by `key`, in `chat` (stored with it when it is new) at `at`; whether it was new: a wait is
  /// kept once, whichever conversation kept it.
  pub fn keep_agent_wait(
    &mut self,
    chat: &Chat,
    (key, wait): (&str, &AgentWait),
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let body = serde_json::to_string(wait).map_err(|error| corrupt("chat wait", error))?;
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT INTO orchestrator_chats (id, title, started_at) VALUES (?1, ?2, ?3) ON CONFLICT (id) DO NOTHING",
      params![chat.id().to_string(), chat.title(), chat.started().unix_millis()],
    )?;
    let kept = transaction.execute(
      "INSERT INTO orchestrator_entries (chat_id, kind, at, body, wait_key) VALUES (?1, 'wait', ?2, ?3, ?4)
       ON CONFLICT (wait_key) DO NOTHING",
      params![chat.id().to_string(), at.unix_millis(), body, key],
    )?;
    transaction.commit()?;
    Ok(kept == 1)
  }

  /// The waits of every conversation that start a turn by themselves (`automatic`) and have none, oldest first.
  pub fn automatic_waits_without_turn(&self) -> Result<Vec<KeptWait>, StoreError> {
    self.kept_waits("message_id IS NULL AND json_extract(body, '$.automatic') = 1")
  }

  /// The waits of every conversation whose turn thinks now, oldest first.
  pub fn waits_thinking(&self) -> Result<Vec<KeptWait>, StoreError> {
    self.kept_waits("turn = 'thinking'")
  }

  /// The waits that hold for `condition`, a fixed SQL condition on their entries, oldest first.
  fn kept_waits(&self, condition: &'static str) -> Result<Vec<KeptWait>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT chat_id, seq, wait_key, body FROM orchestrator_entries WHERE kind = 'wait' AND {condition} ORDER BY seq"
    ))?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, i64>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, String>(3)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(chat, seq, key, body)| {
        Ok(KeptWait {
          chat: parse("chat id", &chat)?,
          seq,
          key,
          wait: wait_of(&body)?,
        })
      })
      .collect()
  }

  /// Gives wait or daily report `seq` of `chat` its turn, thinking, under `message`; only while it has none and no
  /// other turn of the conversation thinks. Whether it was given.
  pub fn start_wait_turn(&mut self, (chat, seq): (ChatId, i64), message: ChatMessageId) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE orchestrator_entries SET message_id = ?3, turn = 'thinking'
       WHERE seq = ?2 AND chat_id = ?1 AND kind IN ('wait', 'report') AND message_id IS NULL
         AND NOT EXISTS (SELECT 1 FROM orchestrator_entries WHERE chat_id = ?1 AND turn = 'thinking')",
      params![chat.to_string(), seq, message.to_string()],
    )?;
    Ok(changed == 1)
  }

  /// The conversation whose last entry is the newest; `None` while the project has none.
  pub fn latest_chat(&self) -> Result<Option<ChatId>, StoreError> {
    let id = self
      .connection
      .query_row(
        "SELECT c.id FROM orchestrator_chats c LEFT JOIN orchestrator_entries e ON e.chat_id = c.id
         GROUP BY c.id ORDER BY COALESCE(MAX(e.at), c.started_at) DESC, c.id DESC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
      )
      .optional()?;
    id.map(|id| parse("chat id", &id)).transpose()
  }
}
