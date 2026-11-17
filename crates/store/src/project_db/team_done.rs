//! The Done a merged team run asks for its task (`schema::team_done`, OA-30): recorded with the run's merge, then
//! settled once by what the team server answered. A refusal keeps the reason the workspace gives it.

use orchester_core::{
  id::{RunId, TaskId},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The Done a merged team run asks for its task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedDone {
  pub run: RunId,
  /// The task on the team server.
  pub team_task: TaskId,
  pub merged_at: Timestamp,
  pub state: MergedDoneState,
}

/// Where a merged team run's Done stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergedDoneState {
  /// Not answered by the server yet: sent at the next round.
  Pending,
  /// The server has the task Done.
  Sent,
  /// The server did not set it Done, for `reason`: its refusal code, or the status that stood.
  NotSent { reason: String },
}

impl ProjectDb {
  /// The Done of merged team runs the server has not answered yet, oldest merge first.
  pub fn pending_merged_done(&self) -> Result<Vec<MergedDone>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("{SELECT} WHERE state = 'pending' ORDER BY merged_at, run_id"))?;
    let rows = statement.query_map([], read)?;
    rows.map(|row| row?).collect()
  }

  /// The Done team run `run`'s merge asked for; `None` for a run that is no merged team run.
  pub fn merged_done(&self, run: RunId) -> Result<Option<MergedDone>, StoreError> {
    self
      .connection
      .query_row(&format!("{SELECT} WHERE run_id = ?1"), [run.to_string()], read)
      .optional()?
      .transpose()
  }

  /// The server has the task of merged team run `run` Done. A Done settled before stays as it was.
  pub fn merged_done_sent(&mut self, run: RunId, at: Timestamp) -> Result<(), StoreError> {
    settle(&self.connection, run, None, at)
  }

  /// The server did not set the task of merged team run `run` Done, for `reason`. A Done settled before stays as it
  /// was.
  pub fn merged_done_not_sent(&mut self, run: RunId, reason: &str, at: Timestamp) -> Result<(), StoreError> {
    settle(&self.connection, run, Some(reason), at)
  }
}

/// Records that merged team run `run`, merged at `at`, asks for its task's Done; nothing for a run with no team record.
pub(super) fn record(connection: &Connection, run: RunId, at: Timestamp) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO team_merged_done (run_id, team_task_id, merged_at, state) \
     SELECT run_id, team_task_id, ?2, 'pending' FROM team_runs WHERE run_id = ?1 \
     ON CONFLICT (run_id) DO NOTHING",
    params![run.to_string(), at.unix_millis()],
  )?;
  Ok(())
}

fn settle(connection: &Connection, run: RunId, reason: Option<&str>, at: Timestamp) -> Result<(), StoreError> {
  let state = if reason.is_some() { "not_sent" } else { "sent" };
  connection.execute(
    "UPDATE team_merged_done SET state = ?2, reason = ?3, settled_at = ?4 WHERE run_id = ?1 AND state = 'pending'",
    params![run.to_string(), state, reason, at.unix_millis()],
  )?;
  Ok(())
}

const SELECT: &str = "SELECT run_id, team_task_id, merged_at, state, reason FROM team_merged_done";

fn read(row: &Row<'_>) -> rusqlite::Result<Result<MergedDone, StoreError>> {
  let (run, task, at, state, reason): (String, String, i64, String, Option<String>) =
    (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?);
  Ok(merged_done((&run, &task), at, &state, reason))
}

fn merged_done(
  (run, task): (&str, &str),
  at: i64,
  state: &str,
  reason: Option<String>,
) -> Result<MergedDone, StoreError> {
  let state = match (state, reason) {
    ("pending", None) => MergedDoneState::Pending,
    ("sent", None) => MergedDoneState::Sent,
    ("not_sent", Some(reason)) => MergedDoneState::NotSent { reason },
    (other, _) => return Err(corrupt("merged team run's Done", other)),
  };
  Ok(MergedDone {
    run: parse("run id", run)?,
    team_task: parse("task id", task)?,
    merged_at: Timestamp::from_unix_millis(at),
    state,
  })
}
