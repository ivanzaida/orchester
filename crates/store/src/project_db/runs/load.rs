//! Reading runs back. Rows are rebuilt through [`Run::restore`] and events through the domain
//! deserializers, so a damaged row surfaces as `StoreError::Corrupt`.

use std::num::{NonZeroU32, NonZeroU64};

use orchester_core::{
  id::RunId,
  money::{Currency, Money},
  revision::Revision,
  run::{
    Attempt, BranchName, Checkpoint, EventSequence, Run, RunCreated, RunEvent, RunEventKind, StepExecution, StoredRun,
  },
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Params, Row};

use super::{
  EVENT_SCHEMA, SNAPSHOT_SCHEMA,
  columns::{parse_attempt_reason, parse_attempt_status, parse_run_status, parse_step_status, step_key},
  run_number,
};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, parse},
};

const SELECT_RUNS: &str = "SELECT id, task_id, number, snapshot, branch, base_revision, currency, budget_micros, \
                           spent_micros, status, status_reason, failed_step, last_sequence, created_at FROM runs";

pub(super) fn load_runs(connection: &Connection, clause: &str, params: impl Params) -> Result<Vec<Run>, StoreError> {
  let mut statement = connection.prepare(&format!("{SELECT_RUNS} {clause}"))?;
  let rows = statement
    .query_map(params, RunRow::read)?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(|row| row.into_run(connection)).collect()
}

pub(super) fn load_events(connection: &Connection, run: RunId) -> Result<Vec<RunEvent>, StoreError> {
  let mut statement =
    connection.prepare("SELECT sequence, at, kind, payload FROM run_events WHERE run_id = ?1 ORDER BY sequence")?;
  let rows = statement
    .query_map([run.to_string()], |row| {
      Ok((
        row.get::<_, i64>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(sequence, at, kind, payload)| {
      let decoded: RunEventKind = decode_json("run event", EVENT_SCHEMA, &payload)?;
      if decoded.name() != kind {
        return Err(corrupt(
          "run event",
          format!("kind `{kind}` holds a `{}` payload", decoded.name()),
        ));
      }
      Ok(RunEvent::new(
        run,
        sequence_value(sequence)?,
        Timestamp::from_unix_millis(at),
        decoded,
      ))
    })
    .collect()
}

pub(super) fn load_checkpoint(connection: &Connection, run: RunId) -> Result<Option<Checkpoint>, StoreError> {
  let row: Option<(String, String, String, i64)> = connection
    .query_row(
      "SELECT id, step_execution_id, revision, created_at FROM checkpoints WHERE run_id = ?1 \
       ORDER BY sequence DESC LIMIT 1",
      [run.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .optional()?;
  let Some((id, execution, revision, created_at)) = row else {
    return Ok(None);
  };
  Ok(Some(Checkpoint::new(
    parse("checkpoint id", &id)?,
    parse("step execution id", &execution)?,
    Revision::new(&revision).map_err(|error| corrupt("checkpoint revision", error))?,
    Timestamp::from_unix_millis(created_at),
  )))
}

fn load_attempts(connection: &Connection, run: &str) -> Result<Vec<Attempt>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT id, number, reason, failed_step, return_to, status FROM attempts WHERE run_id = ?1 ORDER BY number",
  )?;
  type AttemptColumns = (String, i64, String, Option<String>, Option<String>, String);
  let rows = statement
    .query_map([run], |row| {
      Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
      ))
    })?
    .collect::<Result<Vec<AttemptColumns>, _>>()?;
  rows
    .into_iter()
    .map(|(id, number, reason, failed_step, return_to, status)| {
      let number = u32::try_from(number)
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or_else(|| corrupt("attempt number", number))?;
      Ok(Attempt::new(
        parse("attempt id", &id)?,
        number,
        parse_attempt_reason(&reason, failed_step.as_deref(), return_to.as_deref())?,
        parse_attempt_status(&status)?,
        load_executions(connection, &id)?,
      ))
    })
    .collect()
}

/// Executions in insertion order, which is pipeline order within an attempt.
fn load_executions(connection: &Connection, attempt: &str) -> Result<Vec<StepExecution>, StoreError> {
  let mut statement = connection
    .prepare("SELECT id, step_key, status, wait_reason FROM step_executions WHERE attempt_id = ?1 ORDER BY rowid")?;
  let rows = statement
    .query_map([attempt], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, Option<String>>(3)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(id, step, status, wait_reason)| {
      Ok(StepExecution::new(
        parse("step execution id", &id)?,
        step_key(&step)?,
        parse_step_status(&status, wait_reason.as_deref())?,
      ))
    })
    .collect()
}

fn sequence_value(sequence: i64) -> Result<EventSequence, StoreError> {
  u64::try_from(sequence)
    .ok()
    .and_then(NonZeroU64::new)
    .map(EventSequence::new)
    .ok_or_else(|| corrupt("run event sequence", sequence))
}

struct RunRow {
  id: String,
  task: String,
  number: i64,
  snapshot: String,
  branch: String,
  base: String,
  currency: String,
  budget_micros: i64,
  spent_micros: i64,
  status: String,
  status_reason: Option<String>,
  failed_step: Option<String>,
  last_sequence: i64,
  created_at: i64,
}

impl RunRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      task: row.get(1)?,
      number: row.get(2)?,
      snapshot: row.get(3)?,
      branch: row.get(4)?,
      base: row.get(5)?,
      currency: row.get(6)?,
      budget_micros: row.get(7)?,
      spent_micros: row.get(8)?,
      status: row.get(9)?,
      status_reason: row.get(10)?,
      failed_step: row.get(11)?,
      last_sequence: row.get(12)?,
      created_at: row.get(13)?,
    })
  }

  fn into_run(self, connection: &Connection) -> Result<Run, StoreError> {
    let id: RunId = parse("run id", &self.id)?;
    let currency = Currency::new(&self.currency).map_err(|error| corrupt("run currency", error))?;
    let created = RunCreated {
      task: parse("task id", &self.task)?,
      number: run_number(self.number)?,
      snapshot: decode_json("run snapshot", SNAPSHOT_SCHEMA, &self.snapshot)?,
      branch: BranchName::new(&self.branch).map_err(|error| corrupt("run branch", error))?,
      base: Revision::new(&self.base).map_err(|error| corrupt("run base revision", error))?,
      budget: Money::from_micros(self.budget_micros, currency),
    };
    let stored = StoredRun {
      id,
      created,
      created_at: Timestamp::from_unix_millis(self.created_at),
      status: parse_run_status(&self.status, self.status_reason.as_deref(), self.failed_step.as_deref())?,
      spent: Money::from_micros(self.spent_micros, currency),
      attempts: load_attempts(connection, &self.id)?,
      last_checkpoint: load_checkpoint(connection, id)?,
      last_sequence: sequence_value(self.last_sequence)?,
    };
    Run::restore(stored).map_err(|error| corrupt("run", error))
  }
}
