//! How each tool request was answered by its run's approval mode, and why one came to the person
//! (`orchester_core::run::RequestAnswer`, `CameToPerson`, `docs/design/approval-modes.md`): written by the runner, read
//! by the app and Orchester's MCP reads.

use orchester_core::{
  id::RunId,
  run::{AnswerRecord, CameToPerson, RequestAnswer},
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{decode_json, encode_json},
};

const ANSWER: &str = "tool request answer";
const CAME: &str = "tool request came to the person";
const SCHEMA: u32 = 1;

impl ProjectDb {
  /// Records how request `id` of `run` was answered, once its answer is claimed.
  pub fn record_request_answer(&mut self, run: RunId, id: &str, answer: &RequestAnswer) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE tool_approvals SET answered_as = ?3 WHERE run_id = ?1 AND id = ?2",
      params![run.to_string(), id, encode_json(ANSWER, SCHEMA, answer)?],
    )?;
    Ok(())
  }

  /// Records why request `id` of `run`, still waiting, came to the person; `None` while it no longer waits for them
  /// (the run dropped to Manual and its rules look at it again).
  pub fn record_came_to_person(&mut self, run: RunId, id: &str, came: Option<&CameToPerson>) -> Result<(), StoreError> {
    let stored = came.map(|came| encode_json(CAME, SCHEMA, came)).transpose()?;
    self.connection.execute(
      "UPDATE tool_approvals SET came_to_person = ?3 WHERE run_id = ?1 AND id = ?2 AND state = 'pending'",
      params![run.to_string(), id, stored],
    )?;
    Ok(())
  }

  /// Why request `id` of `run` came to the person; `None` when it did not, or before reasons were recorded.
  pub fn came_to_person(&self, run: RunId, id: &str) -> Result<Option<CameToPerson>, StoreError> {
    let stored: Option<Option<String>> = self
      .connection
      .query_row(
        "SELECT came_to_person FROM tool_approvals WHERE run_id = ?1 AND id = ?2",
        params![run.to_string(), id],
        |row| row.get(0),
      )
      .optional()?;
    stored
      .flatten()
      .map(|text| decode_json(CAME, SCHEMA, &text))
      .transpose()
  }

  /// How each request of `run` was answered and whether it came to the person, oldest first.
  pub fn request_answers(&self, run: RunId) -> Result<Vec<AnswerRecord>, StoreError> {
    let mut query = self.connection.prepare(
      "SELECT id, answered_as, came_to_person FROM tool_approvals WHERE run_id = ?1 ORDER BY requested_at, id",
    )?;
    let rows = query
      .query_map([run.to_string()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, Option<String>>(1)?,
          row.get::<_, Option<String>>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(request, answer, came)| {
        Ok(AnswerRecord {
          request,
          answer: answer.map(|text| decode_json(ANSWER, SCHEMA, &text)).transpose()?,
          came_to_person: came.map(|text| decode_json(CAME, SCHEMA, &text)).transpose()?,
        })
      })
      .collect()
  }
}
