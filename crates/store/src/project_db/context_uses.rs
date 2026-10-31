//! How full each agent session's context window is (ORC-304): the runner keeps an invocation's latest reading
//! (`invocation_contexts`) and the Orchestrator chat's for the session its turns continue (`orchestrator_chats`), each
//! in place of the one before; screens and automation read them back.

use std::collections::HashMap;

use orchester_core::{
  connection::context_use::ContextUse,
  id::{ChatId, InvocationId, RunId},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse, tokens_column},
};

impl ProjectDb {
  /// Keeps `context` as `invocation`'s latest reading, taken at `at`.
  pub fn save_invocation_context(
    &mut self,
    invocation: InvocationId,
    context: &ContextUse,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let (used, window) = columns(context)?;
    self.connection.execute(
      "INSERT INTO invocation_contexts (invocation_id, used_tokens, window_tokens, estimated, at)
       VALUES (?1, ?2, ?3, ?4, ?5)
       ON CONFLICT (invocation_id) DO UPDATE SET used_tokens = excluded.used_tokens,
         window_tokens = excluded.window_tokens, estimated = excluded.estimated, at = excluded.at",
      params![
        invocation.to_string(),
        used,
        window,
        context.estimated(),
        at.unix_millis()
      ],
    )?;
    Ok(())
  }

  /// The latest reading of each invocation of `run` that reported one.
  pub fn invocation_contexts(&self, run: RunId) -> Result<HashMap<InvocationId, ContextUse>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT c.invocation_id, c.used_tokens, c.window_tokens, c.estimated FROM invocation_contexts c
       JOIN invocations i ON i.id = c.invocation_id WHERE i.run_id = ?1",
    )?;
    let rows = statement
      .query_map([run.to_string()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          (
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, bool>(3)?,
          ),
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(invocation, raw)| Ok((parse("invocation id", &invocation)?, context_of(raw)?)))
      .collect()
  }

  /// Keeps `context` as the latest reading of the session `chat`'s turns continue.
  pub fn set_chat_context(&mut self, chat: ChatId, context: &ContextUse) -> Result<(), StoreError> {
    let (used, window) = columns(context)?;
    self.connection.execute(
      "UPDATE orchestrator_chats SET context_used = ?2, context_window = ?3, context_estimated = ?4 WHERE id = ?1",
      params![chat.to_string(), used, window, context.estimated()],
    )?;
    Ok(())
  }

  /// The latest reading of the session `chat`'s turns continue; `None` while none reported one, or when `chat` is
  /// not kept.
  pub fn chat_context(&self, chat: ChatId) -> Result<Option<ContextUse>, StoreError> {
    let raw = self
      .connection
      .query_row(
        "SELECT context_used, context_window, context_estimated FROM orchestrator_chats WHERE id = ?1",
        [chat.to_string()],
        |row| {
          Ok((
            row.get::<_, Option<i64>>(0)?,
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, Option<bool>>(2)?,
          ))
        },
      )
      .optional()?;
    match raw {
      Some((Some(used), window, Some(estimated))) => context_of((used, window, estimated)).map(Some),
      Some((None, None, None)) | None => Ok(None),
      Some(other) => Err(corrupt("chat context use", format!("{other:?}"))),
    }
  }
}

/// The used and window columns of `context`.
fn columns(context: &ContextUse) -> Result<(i64, Option<i64>), StoreError> {
  Ok((
    tokens_column("context tokens", context.used())?,
    context
      .window()
      .map(|window| tokens_column("context window", window))
      .transpose()?,
  ))
}

/// A reading from its used, window and estimated columns.
fn context_of((used, window, estimated): (i64, Option<i64>, bool)) -> Result<ContextUse, StoreError> {
  let count = |value: i64| u64::try_from(value).map_err(|_| corrupt("context tokens", value));
  let used = count(used)?;
  match (window.map(count).transpose()?, estimated) {
    (Some(window), estimated) => Ok(ContextUse::new(used, window, estimated)),
    (None, false) => Ok(ContextUse::unknown_window(used)),
    (None, true) => Err(corrupt("context use", "an estimated window is missing")),
  }
}
