//! The latest todo list of each step execution and of each turn of the Orchestrator chat
//! (`orchester_core::connection::stream::TodoList`). A step's is the last `todo_list` event of its transcript; a
//! turn's is kept apart, as a chat turn keeps no transcript, each update in place of the one before.

use orchester_core::{
  connection::stream::{AgentEvent, TodoItem, TodoList},
  id::{ChatId, ChatMessageId, RunId, StepExecutionId},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, activity::EVENT_SCHEMA};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const TODO_SCHEMA: u32 = 1;

impl ProjectDb {
  /// The todo list `execution` of `run` left with its latest update; `None` when its agent kept none, or emptied it.
  pub fn step_todo(&self, run: RunId, execution: StepExecutionId) -> Result<Option<TodoList>, StoreError> {
    let payload: Option<String> = self
      .connection
      .query_row(
        "
        SELECT payload FROM agent_events
        WHERE run_id = ?1 AND step_execution_id = ?2 AND kind = 'todo_list'
        ORDER BY sequence DESC LIMIT 1",
        params![run.to_string(), execution.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    let Some(payload) = payload else {
      return Ok(None);
    };
    match decode_json("agent event", EVENT_SCHEMA, &payload)? {
      AgentEvent::TodoList { items } => Ok(TodoList::new(items)),
      other => Err(corrupt(
        "agent event",
        format!("kind `todo_list` holds a `{}` payload", other.name()),
      )),
    }
  }

  /// Keeps `items`, reported at `at`, as the todo list of the turn `message` started, in place of the one before;
  /// none for no items.
  pub fn keep_chat_todo(
    &mut self,
    message: ChatMessageId,
    items: &[TodoItem],
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let Some(list) = TodoList::new(items.to_vec()) else {
      self.connection.execute(
        "DELETE FROM orchestrator_todos WHERE message_id = ?1",
        [message.to_string()],
      )?;
      return Ok(());
    };
    self.connection.execute(
      "INSERT INTO orchestrator_todos (message_id, items, updated_at) VALUES (?1, ?2, ?3)
       ON CONFLICT (message_id) DO UPDATE SET items = excluded.items, updated_at = excluded.updated_at",
      params![
        message.to_string(),
        encode_json("chat todo list", TODO_SCHEMA, &list)?,
        at.unix_millis()
      ],
    )?;
    Ok(())
  }

  /// The todo lists of `chat`'s turns that keep one, by the message that started each, in the order they were kept.
  pub fn chat_todos(&self, chat: ChatId) -> Result<Vec<(ChatMessageId, TodoList)>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT t.message_id, t.items FROM orchestrator_todos t
       JOIN orchestrator_entries e ON e.message_id = t.message_id
       WHERE e.chat_id = ?1 ORDER BY e.seq",
    )?;
    let rows = statement
      .query_map([chat.to_string()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(message, items)| {
        Ok((
          parse("chat message id", &message)?,
          decode_json("chat todo list", TODO_SCHEMA, &items)?,
        ))
      })
      .collect()
  }
}
