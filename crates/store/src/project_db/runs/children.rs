//! What a run keeps in its other tables: its raised step budgets, its permissions, its attempts with their step
//! executions and its latest checkpoint. A listing reads them for every run it selects at once, one statement per
//! table, and parses them per run: a row that cannot be read makes only its own run unreadable.

use std::{
  collections::{BTreeMap, HashMap},
  num::NonZeroU32,
};

use orchester_core::{
  id::ResourceId,
  resource::StepKey,
  revision::Revision,
  run::{Attempt, Checkpoint, RunPermissions, StepExecution},
  time::Timestamp,
  tokens::TokenBudget,
};
use rusqlite::{Connection, Row, ToSql};

use super::{
  PERMISSIONS_SCHEMA,
  columns::{
    AttemptReasonColumns, parse_attempt_reason, parse_attempt_status, parse_step_failure, parse_step_status, step_key,
  },
};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, optional_budget_value, parse},
};

/// `id`, `number`, `reason`, `failed_step`, `return_to`, `note`, `status`, `conflicts` and `profile` of an attempt.
type AttemptColumns = (
  String,
  i64,
  String,
  Option<String>,
  Option<String>,
  Option<String>,
  String,
  Option<String>,
  Option<String>,
);
/// `id`, `step_key`, `status`, `wait_reason`, `failure` and `judged_revision` of a step execution.
type ExecutionColumns = (String, String, String, Option<String>, Option<String>, Option<String>);
/// `id`, `step_execution_id`, `revision` and `created_at` of a checkpoint.
pub(super) type CheckpointColumns = (String, String, String, i64);

/// One run's rows, each as read or with the error reading it gave.
#[derive(Default)]
struct RunRows {
  step_budgets: Vec<rusqlite::Result<(String, Option<i64>)>>,
  permissions: Vec<rusqlite::Result<(String, String)>>,
  attempts: Vec<rusqlite::Result<AttemptColumns>>,
  checkpoint: Option<rusqlite::Result<CheckpointColumns>>,
}

/// A run's rows in its other tables, parsed.
pub(super) struct RunRecords {
  pub(super) step_budgets: BTreeMap<StepKey, Option<TokenBudget>>,
  pub(super) permissions: BTreeMap<ResourceId, RunPermissions>,
  pub(super) attempts: Vec<Attempt>,
  pub(super) last_checkpoint: Option<Checkpoint>,
}

/// The rows of the runs a listing selects, by run id.
pub(super) struct Children {
  runs: HashMap<String, RunRows>,
  /// Each attempt's step executions, by attempt id, in insertion order: pipeline order within an attempt.
  executions: HashMap<String, Vec<rusqlite::Result<ExecutionColumns>>>,
}

impl Children {
  /// The rows of the runs `SELECT id FROM runs {clause}` selects with `params`.
  pub(super) fn read(connection: &Connection, clause: &str, params: &[&dyn ToSql]) -> Result<Self, StoreError> {
    let runs = format!("SELECT id FROM runs {clause}");
    let mut children = Self {
      runs: HashMap::new(),
      executions: HashMap::new(),
    };
    let query = format!("SELECT run_id, step_key, budget_tokens FROM run_step_budgets WHERE run_id IN ({runs})");
    for (run, row) in keyed(connection, &query, params, |row| Ok((row.get(1)?, row.get(2)?)))? {
      children.of(run).step_budgets.push(row);
    }
    let query = format!("SELECT run_id, profile_id, body FROM run_permissions WHERE run_id IN ({runs})");
    for (run, row) in keyed(connection, &query, params, |row| Ok((row.get(1)?, row.get(2)?)))? {
      children.of(run).permissions.push(row);
    }
    let query = format!(
      "SELECT run_id, id, number, reason, failed_step, return_to, note, status, conflicts, profile FROM attempts \
       WHERE run_id IN ({runs}) ORDER BY number"
    );
    for (run, row) in keyed(connection, &query, params, attempt_columns)? {
      children.of(run).attempts.push(row);
    }
    let query = format!(
      "SELECT attempt_id, id, step_key, status, wait_reason, failure, judged_revision FROM step_executions \
       WHERE attempt_id IN (SELECT id FROM attempts WHERE run_id IN ({runs})) ORDER BY rowid"
    );
    for (attempt, row) in keyed(connection, &query, params, execution_columns)? {
      children.executions.entry(attempt).or_default().push(row);
    }
    // SQLite takes the other columns from the row of the greatest sequence: the run's latest checkpoint.
    let query = format!(
      "SELECT run_id, id, step_execution_id, revision, created_at, MAX(sequence) FROM checkpoints \
       WHERE run_id IN ({runs}) GROUP BY run_id"
    );
    for (run, row) in keyed(connection, &query, params, |row| {
      Ok((row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
    })? {
      children.of(run).checkpoint = Some(row);
    }
    Ok(children)
  }

  fn of(&mut self, run: String) -> &mut RunRows {
    self.runs.entry(run).or_default()
  }

  /// The rows of run `run`, parsed; taken, as each run is restored once.
  pub(super) fn take(&mut self, run: &str) -> Result<RunRecords, StoreError> {
    let rows = self.runs.remove(run).unwrap_or_default();
    Ok(RunRecords {
      step_budgets: rows
        .step_budgets
        .into_iter()
        .map(|row| {
          let (step, budget) = row?;
          Ok((step_key(&step)?, optional_budget_value("step budget", budget)?))
        })
        .collect::<Result<_, StoreError>>()?,
      permissions: rows
        .permissions
        .into_iter()
        .map(|row| {
          let (profile, body) = row?;
          Ok((
            parse("profile id", &profile)?,
            decode_json("run permissions", PERMISSIONS_SCHEMA, &body)?,
          ))
        })
        .collect::<Result<_, StoreError>>()?,
      attempts: rows
        .attempts
        .into_iter()
        .map(|row| self.attempt(row?))
        .collect::<Result<_, _>>()?,
      last_checkpoint: rows.checkpoint.transpose()?.map(checkpoint).transpose()?,
    })
  }

  fn attempt(
    &mut self,
    (id, number, reason, failed_step, return_to, note, status, conflicts, profile): AttemptColumns,
  ) -> Result<Attempt, StoreError> {
    let number = u32::try_from(number)
      .ok()
      .and_then(NonZeroU32::new)
      .ok_or_else(|| corrupt("attempt number", number))?;
    let executions = self
      .executions
      .remove(&id)
      .unwrap_or_default()
      .into_iter()
      .map(|row| execution(row?))
      .collect::<Result<_, _>>()?;
    Ok(Attempt::new(
      parse("attempt id", &id)?,
      number,
      parse_attempt_reason(&AttemptReasonColumns {
        reason: &reason,
        failed_step: failed_step.as_deref(),
        return_to: return_to.as_deref(),
        note: note.as_deref(),
        conflicts: conflicts.as_deref(),
        profile: profile.as_deref(),
      })?,
      parse_attempt_status(&status)?,
      executions,
    ))
  }
}

/// The rows `query` returns with `params`, each with the key in its first column; the other columns as `columns` reads
/// them, or the error reading them gave.
fn keyed<T>(
  connection: &Connection,
  query: &str,
  params: &[&dyn ToSql],
  columns: impl Fn(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<(String, rusqlite::Result<T>)>, StoreError> {
  let mut statement = connection.prepare(query)?;
  let rows = statement
    .query_map(params, |row| Ok((row.get::<_, String>(0)?, columns(row))))?
    .collect::<Result<Vec<_>, _>>()?;
  Ok(rows)
}

fn attempt_columns(row: &Row<'_>) -> rusqlite::Result<AttemptColumns> {
  Ok((
    row.get(1)?,
    row.get(2)?,
    row.get(3)?,
    row.get(4)?,
    row.get(5)?,
    row.get(6)?,
    row.get(7)?,
    row.get(8)?,
    row.get(9)?,
  ))
}

fn execution_columns(row: &Row<'_>) -> rusqlite::Result<ExecutionColumns> {
  Ok((
    row.get(1)?,
    row.get(2)?,
    row.get(3)?,
    row.get(4)?,
    row.get(5)?,
    row.get(6)?,
  ))
}

fn execution((id, step, status, wait_reason, failure, judged): ExecutionColumns) -> Result<StepExecution, StoreError> {
  let failure = failure.as_deref().map(parse_step_failure).transpose()?;
  let judged = judged
    .as_deref()
    .map(Revision::new)
    .transpose()
    .map_err(|error| corrupt("judged revision", error))?;
  Ok(
    StepExecution::new(
      parse("step execution id", &id)?,
      step_key(&step)?,
      parse_step_status(&status, wait_reason.as_deref())?,
    )
    .with_failure(failure)
    .with_judged(judged),
  )
}

/// The checkpoint of these columns.
pub(super) fn checkpoint((id, execution, revision, created_at): CheckpointColumns) -> Result<Checkpoint, StoreError> {
  Ok(Checkpoint::new(
    parse("checkpoint id", &id)?,
    parse("step execution id", &execution)?,
    Revision::new(&revision).map_err(|error| corrupt("checkpoint revision", error))?,
    Timestamp::from_unix_millis(created_at),
  ))
}
