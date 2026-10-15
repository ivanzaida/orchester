//! What the Orchestrator chat's turns spent (`orchestrator_usage`): one record per usage its CLI reported, written by
//! the runner and counted toward the project's daily budget with the runs' turns. The chat takes no agent slot.

use orchester_core::{
  id::{ChatId, ChatMessageId},
  run::TokenUsage,
  time::Timestamp,
  tokens::TokensUsed,
};
use rusqlite::{Connection, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, tokens_column},
};

impl ProjectDb {
  /// Keeps `usage`, which the turn answering `message` of `chat` reported at `at`.
  pub fn record_chat_usage(
    &mut self,
    (chat, message): (ChatId, ChatMessageId),
    usage: TokenUsage,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO orchestrator_usage (chat_id, message_id, at, input_tokens, cached_input_tokens, output_tokens) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![
        chat.to_string(),
        message.to_string(),
        at.unix_millis(),
        tokens_column("chat tokens", usage.input)?,
        tokens_column("chat tokens", usage.cached_input)?,
        tokens_column("chat tokens", usage.output)?,
      ],
    )?;
    Ok(())
  }

  /// What the turn answering `message` spent, counted as the daily budget counts it.
  pub fn chat_turn_tokens(&self, message: ChatMessageId) -> Result<TokensUsed, StoreError> {
    counted(&self.connection, "WHERE message_id = ?1", &message.to_string())
  }
}

/// What the chat's turns spent from `since` on, counted as the daily budget counts it.
pub(super) fn counted_since(connection: &Connection, since: Timestamp) -> Result<TokensUsed, StoreError> {
  counted(connection, "WHERE at >= ?1", &since.unix_millis())
}

fn counted(connection: &Connection, filter: &str, value: &dyn rusqlite::ToSql) -> Result<TokensUsed, StoreError> {
  let mut statement = connection.prepare(&format!(
    "SELECT input_tokens, cached_input_tokens, output_tokens FROM orchestrator_usage {filter}"
  ))?;
  let rows = statement
    .query_map([value], |row| {
      Ok([row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?])
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|tokens| usage(tokens).map(TokenUsage::counted_tokens))
    .sum()
}

fn usage([input, cached_input, output]: [i64; 3]) -> Result<TokenUsage, StoreError> {
  let count = |value: i64| u64::try_from(value).map_err(|_| corrupt("chat tokens", value));
  Ok(TokenUsage {
    input: count(input)?,
    cached_input: count(cached_input)?,
    output: count(output)?,
  })
}
