//! A run's permission events (`orchester_core::run::PermissionEvent`): appended as its permissions menu changes how
//! the run answers its requests, or refuses automation's asking to widen it; read for its Conversation's event lines.

use orchester_core::{
  id::RunId,
  run::{PermissionEvent, PermissionEventKind},
  time::Timestamp,
};
use rusqlite::params;

use super::{
  ProjectDb,
  questions::{answerer, read_answerer},
};
use crate::{
  StoreError,
  rows::{decode_json, encode_json},
};

const EVENT: &str = "run permission event";
/// The stored form of [`PermissionEventKind`].
const EVENT_SCHEMA: u32 = 1;

impl ProjectDb {
  /// Appends `event` to `run`'s permission events.
  pub fn record_permission_event(&mut self, run: RunId, event: &PermissionEvent) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO run_permission_events (run_id, at, by, event) VALUES (?1, ?2, ?3, ?4)",
      params![
        run.to_string(),
        event.at.unix_millis(),
        answerer(event.by),
        encode_json(EVENT, EVENT_SCHEMA, &event.kind)?,
      ],
    )?;
    Ok(())
  }

  /// `run`'s permission events, oldest first.
  pub fn permission_events(&self, run: RunId) -> Result<Vec<PermissionEvent>, StoreError> {
    let mut query = self
      .connection
      .prepare("SELECT at, by, event FROM run_permission_events WHERE run_id = ?1 ORDER BY id")?;
    let rows = query
      .query_map([run.to_string()], |row| {
        Ok((
          row.get::<_, i64>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(at, by, event)| {
        Ok(PermissionEvent {
          at: Timestamp::from_unix_millis(at),
          by: read_answerer("permission event author", Some(&by))?,
          kind: decode_json::<PermissionEventKind>(EVENT, EVENT_SCHEMA, &event)?,
        })
      })
      .collect()
  }
}
