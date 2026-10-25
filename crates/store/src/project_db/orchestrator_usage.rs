//! What the Orchestrator chat's turns spent (`orchestrator_usage`): one record per usage its CLI reported, with the
//! account the turn ran on, written by the runner and counted toward the project's daily budget with the runs' turns.
//! The chat takes no agent slot.

use orchester_core::{
  id::{AgentAccountId, ChatId, ChatMessageId},
  run::TokenUsage,
  time::Timestamp,
  tokens::TokensUsed,
};
use rusqlite::{Connection, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse, tokens_column},
};

impl ProjectDb {
  /// Keeps `usage`, which the turn answering `message` of `chat` on `account` reported at `at`.
  pub fn record_chat_usage(
    &mut self,
    (chat, message): (ChatId, ChatMessageId),
    (account, usage): (AgentAccountId, TokenUsage),
    at: Timestamp,
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO orchestrator_usage (chat_id, message_id, at, input_tokens, cached_input_tokens, output_tokens, \
       account_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        chat.to_string(),
        message.to_string(),
        at.unix_millis(),
        tokens_column("chat tokens", usage.input)?,
        tokens_column("chat tokens", usage.cached_input)?,
        tokens_column("chat tokens", usage.output)?,
        account.to_string(),
      ],
    )?;
    Ok(())
  }

  /// What the turn answering `message` spent, counted as the daily budget counts it.
  pub fn chat_turn_tokens(&self, message: ChatMessageId) -> Result<TokensUsed, StoreError> {
    counted(&self.connection, "WHERE message_id = ?1", &message.to_string())
  }

  /// What the turn answering `message` reported it spent, record by record, each with the account it ran on; `None`
  /// for usage recorded before the account was kept.
  pub fn chat_turn_usage(
    &self,
    message: ChatMessageId,
  ) -> Result<Vec<(Option<AgentAccountId>, TokenUsage)>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT account_id, input_tokens, cached_input_tokens, output_tokens FROM orchestrator_usage \
       WHERE message_id = ?1 ORDER BY id",
    )?;
    let rows = statement
      .query_map([message.to_string()], |row| {
        Ok((
          row.get::<_, Option<String>>(0)?,
          [row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?],
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(account, tokens)| {
        let account = account
          .map(|account| parse::<AgentAccountId>("account id", &account))
          .transpose()?;
        Ok((account, usage(tokens)?))
      })
      .collect()
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
