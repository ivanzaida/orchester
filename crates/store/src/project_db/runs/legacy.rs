//! Run events written before budgets were tokens (event schema 1), upgraded as they are read.
//! Stored events are append-only and are never rewritten, so the migration to token budgets
//! (see `schema.rs`) leaves them as they were and this module reads them the way the migrated
//! run rows describe the run:
//!
//! - `created` carried a money budget and no time limit: it is read with the run's converted creation limits
//!   (`initial_budget_tokens`, `initial_max_duration_ms`).
//! - `paused` had no reason: every pause then was a person's (or closing's) request.
//! - `spend_recorded` was never written by a runner; a log holding one cannot be read with token budgets and is
//!   reported as damaged rather than guessed at.

use rusqlite::Connection;
use serde_json::{Value, json};

use crate::{StoreError, rows::corrupt};

/// The schema events had while budgets were money.
pub(super) const MONEY_SCHEMA: u32 = 1;

/// The creation limits of a migrated run, as its row holds them.
pub(super) struct CreationLimits {
  budget_tokens: i64,
  max_duration_ms: i64,
}

impl CreationLimits {
  pub(super) fn of(connection: &Connection, run: &str) -> Result<Self, StoreError> {
    Ok(connection.query_row(
      "SELECT initial_budget_tokens, initial_max_duration_ms FROM runs WHERE id = ?1",
      [run],
      |row| {
        Ok(Self {
          budget_tokens: row.get(0)?,
          max_duration_ms: row.get(1)?,
        })
      },
    )?)
  }
}

/// The schema 1 body of a `kind` event as the current schema reads it.
pub(super) fn upgrade(kind: &str, mut body: Value, limits: &CreationLimits) -> Result<Value, StoreError> {
  let Some(fields) = body.as_object_mut() else {
    return Err(corrupt("run event", "a schema 1 event is not an object"));
  };
  match kind {
    "created" => {
      fields.insert("budget".to_owned(), json!(limits.budget_tokens));
      fields.insert("max_duration".to_owned(), json!(limits.max_duration_ms));
    }
    "paused" => {
      fields.insert("reason".to_owned(), json!("requested"));
    }
    "spend_recorded" => {
      return Err(corrupt(
        "run event",
        "a money spending event cannot be read now that budgets are tokens",
      ));
    }
    _ => {}
  }
  Ok(body)
}
