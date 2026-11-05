//! Durable test progress, with a stop flag separate from worker updates.
use orchester_core::{
  id::{DecisionConnectionId, IdempotencyKey},
  run::review::{DecisionTestReport, DecisionTestStatus},
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{decode_json, encode_json},
};

impl ProjectDb {
  /// Claims the project's single test slot. Replayed requests and overlapping tests cannot spend twice.
  pub fn begin_decision_test(&mut self, report: &DecisionTestReport) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "INSERT INTO decision_tests (id, connection_id, active, report) VALUES (?1, ?2, 1, ?3) ON CONFLICT DO NOTHING",
      params![
        report.id.to_string(),
        report.connection.to_string(),
        encode_json("decision test", 1, report)?
      ],
    )?;
    Ok(changed == 1)
  }

  pub fn save_decision_test(&mut self, report: &DecisionTestReport) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE decision_tests SET report = ?2, active = ?3 WHERE id = ?1",
      params![
        report.id.to_string(),
        encode_json("decision test", 1, report)?,
        report.status == DecisionTestStatus::Running
      ],
    )?;
    Ok(())
  }

  pub fn decision_test(&self, id: IdempotencyKey) -> Result<Option<DecisionTestReport>, StoreError> {
    let body: Option<String> = self
      .connection
      .query_row(
        "SELECT report FROM decision_tests WHERE id = ?1",
        [id.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    body.map(|body| decode_json("decision test", 1, &body)).transpose()
  }

  pub fn latest_decision_test(
    &self,
    connection: DecisionConnectionId,
  ) -> Result<Option<DecisionTestReport>, StoreError> {
    let body: Option<String> = self
      .connection
      .query_row(
        "SELECT report FROM decision_tests WHERE connection_id = ?1 ORDER BY rowid DESC LIMIT 1",
        [connection.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    body.map(|body| decode_json("decision test", 1, &body)).transpose()
  }

  pub fn stop_decision_test(&mut self, id: IdempotencyKey) -> Result<bool, StoreError> {
    Ok(
      self.connection.execute(
        "UPDATE decision_tests SET stop_requested = 1 WHERE id = ?1 AND active = 1",
        [id.to_string()],
      )? == 1,
    )
  }

  pub fn decision_test_stop_requested(&self, id: IdempotencyKey) -> Result<bool, StoreError> {
    // A deleted connection deletes its tests; the worker must stop sending its requests too.
    let stopped: Option<bool> = self
      .connection
      .query_row(
        "SELECT stop_requested FROM decision_tests WHERE id = ?1",
        [id.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    Ok(stopped.unwrap_or(true))
  }

  /// A new runner never resumes a comparison without another explicit press.
  pub fn interrupt_decision_tests(&mut self) -> Result<(), StoreError> {
    let mut query = self
      .connection
      .prepare("SELECT report FROM decision_tests WHERE active = 1")?;
    let bodies = query
      .query_map([], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    drop(query);
    for body in bodies {
      let mut report: DecisionTestReport = decode_json("decision test", 1, &body)?;
      report.unknown_usage += report.in_flight;
      report.in_flight = 0;
      report.status = DecisionTestStatus::Interrupted;
      self.save_decision_test(&report)?;
    }
    Ok(())
  }
}
