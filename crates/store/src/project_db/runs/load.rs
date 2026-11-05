//! Reading runs back. Rows are rebuilt through [`Run::restore`] and events through the domain
//! deserializers, so a damaged row surfaces as `StoreError::Corrupt`. Each run is restored on its own:
//! a run whose record cannot be read back is listed as an [`UnreadableRun`] beside the others.

use std::{collections::HashMap, num::NonZeroU64};

use orchester_core::{
  id::{RunId, TaskId},
  revision::Revision,
  run::{
    BranchName, Checkpoint, EventSequence, ListedRun, Run, RunCreated, RunEvent, RunEventKind, StoredRun, UnreadableRun,
  },
  time::Timestamp,
  tokens::TokensUsed,
};
use rusqlite::{Connection, OptionalExtension, Row, ToSql};

use super::{
  EVENT_SCHEMA, SNAPSHOT_SCHEMA,
  children::{self, Children, RunRecords},
  columns::parse_run_status,
  legacy::{self, CreationLimits},
  run_number,
};
use crate::{
  RunStoreError, StoreError,
  project_db::ProjectDb,
  rows::{corrupt, decode_body, decode_document, decode_json, optional_budget_value, optional_duration_value, parse},
};

const SELECT_RUNS: &str = "SELECT id, task_id, number, snapshot, branch, base_revision, initial_budget_tokens, \
                           initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens, pause_requested, \
                           status, status_reason, failed_step, limit_execution, last_sequence, created_at, \
                           base_branch, failure_reason, base_source, base_module FROM runs";

impl ProjectDb {
  /// Every run of the project, by task, each task's by number: [`RunStore::runs_of_task`] of every task, read in a
  /// constant number of statements.
  ///
  /// [`RunStore::runs_of_task`]: orchester_core::store::RunStore::runs_of_task
  pub fn runs_by_task(&self) -> Result<HashMap<TaskId, Vec<ListedRun<RunStoreError>>>, RunStoreError> {
    let mut by_task: HashMap<TaskId, Vec<_>> = HashMap::new();
    for run in load_runs(&self.connection, "ORDER BY number", &[])? {
      let task = match &run {
        Ok(run) => run.task(),
        Err(unreadable) => unreadable.task,
      };
      by_task.entry(task).or_default().push(run);
    }
    Ok(by_task)
  }

  /// When each run's last event was recorded, in one statement; a run without events is left out. Reads no event's
  /// payload: [`RunStore::events`]`.last()` of each run, by its time alone.
  ///
  /// [`RunStore::events`]: orchester_core::store::RunStore::events
  pub fn last_event_times(&self) -> Result<HashMap<RunId, Timestamp>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT runs.id, (SELECT at FROM run_events WHERE run_id = runs.id ORDER BY sequence DESC LIMIT 1) FROM runs",
    )?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .filter_map(|(run, at)| Some((run, at?)))
      .map(|(run, at)| Ok((parse("run id", &run)?, Timestamp::from_unix_millis(at))))
      .collect()
  }
}

/// The runs `clause` selects with `params`, each restored on its own. Only a run whose id or task cannot be read
/// fails the listing: it cannot be told which run it is.
///
/// A run is stored in several tables (its row, attempts, step executions, checkpoints, events), read
/// for every selected run at once ([`Children`]) by separate statements in one [consistent
/// read](super::super::consistent::read), so a run row is never paired with rows written after it (the
/// runner recording an attempt while the desktop reads). `clause` is read again in those statements'
/// `IN (SELECT id FROM runs {clause})`, so it names only the columns of `runs`.
pub(super) fn load_runs(
  connection: &Connection,
  clause: &str,
  params: &[&dyn ToSql],
) -> Result<Vec<ListedRun<RunStoreError>>, StoreError> {
  super::super::consistent::read(connection, || read_runs(connection, clause, params))
}

fn read_runs(
  connection: &Connection,
  clause: &str,
  params: &[&dyn ToSql],
) -> Result<Vec<ListedRun<RunStoreError>>, StoreError> {
  let mut statement = connection.prepare(&format!("{SELECT_RUNS} {clause}"))?;
  let rows = statement
    .query_map(params, |row| Ok((RunIdentity::read(row)?, RunRow::read(row))))?
    .collect::<Result<Vec<_>, _>>()?;
  if rows.is_empty() {
    return Ok(Vec::new());
  }
  let mut children = Children::read(connection, clause, params)?;
  rows
    .into_iter()
    .map(|(identity, row)| {
      let id: RunId = parse("run id", &identity.id)?;
      let task: TaskId = parse("task id", &identity.task)?;
      let restored = row
        .map_err(StoreError::from)
        .and_then(|row| row.into_run(children.take(&identity.id)?, (id, task, identity.number)));
      Ok(restored.map_err(|source| UnreadableRun {
        id,
        task,
        number: run_number(identity.number).ok(),
        error: RunStoreError::Unreadable { run: id, source },
      }))
    })
    .collect()
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
  let mut creation = None;
  rows
    .into_iter()
    .map(|(sequence, at, kind, payload)| {
      let (schema, body) = decode_document("run event", &payload)?;
      let body = match schema {
        EVENT_SCHEMA => body,
        legacy::MONEY_SCHEMA => {
          let limits = match &mut creation {
            Some(limits) => limits,
            None => creation.insert(CreationLimits::of(connection, &run.to_string())?),
          };
          legacy::upgrade(&kind, body, limits)?
        }
        other => return Err(corrupt("run event", format!("schema {other} is not supported"))),
      };
      let decoded: RunEventKind = decode_body("run event", body)?;
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
  let row: Option<children::CheckpointColumns> = connection
    .query_row(
      "SELECT id, step_execution_id, revision, created_at FROM checkpoints WHERE run_id = ?1 \
       ORDER BY sequence DESC LIMIT 1",
      [run.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .optional()?;
  row.map(children::checkpoint).transpose()
}

pub(in crate::project_db) fn sequence_value(sequence: i64) -> Result<EventSequence, StoreError> {
  u64::try_from(sequence)
    .ok()
    .and_then(NonZeroU64::new)
    .map(EventSequence::new)
    .ok_or_else(|| corrupt("run event sequence", sequence))
}

/// The columns that tell which run a row is.
struct RunIdentity {
  id: String,
  task: String,
  number: i64,
}

impl RunIdentity {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      task: row.get(1)?,
      number: row.get(2)?,
    })
  }
}

/// The other columns of a run's row.
struct RunRow {
  snapshot: String,
  branch: String,
  base: String,
  /// Creation budget and time limit, then the ones in force; `None` is no limit.
  limits: [Option<i64>; 4],
  used_tokens: i64,
  pause_requested: bool,
  status: String,
  status_reason: Option<String>,
  failed_step: Option<String>,
  limit_execution: Option<String>,
  last_sequence: i64,
  created_at: i64,
  base_branch: Option<String>,
  failure_reason: Option<String>,
  base_source: (String, Option<String>),
}

impl RunRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      snapshot: row.get(3)?,
      branch: row.get(4)?,
      base: row.get(5)?,
      limits: [row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?],
      used_tokens: row.get(10)?,
      pause_requested: row.get(11)?,
      status: row.get(12)?,
      status_reason: row.get(13)?,
      failed_step: row.get(14)?,
      limit_execution: row.get(15)?,
      last_sequence: row.get(16)?,
      created_at: row.get(17)?,
      base_branch: row.get(18)?,
      failure_reason: row.get(19)?,
      base_source: (row.get(20)?, row.get(21)?),
    })
  }

  /// The run `id` of `task`, numbered `number`, with these columns and its other rows, `records`.
  fn into_run(self, records: RunRecords, (id, task, number): (RunId, TaskId, i64)) -> Result<Run, StoreError> {
    let [initial_budget, initial_duration, budget, duration] = self.limits;
    let (source, module) = &self.base_source;
    let created = RunCreated {
      task,
      number: run_number(number)?,
      snapshot: decode_json("run snapshot", SNAPSHOT_SCHEMA, &self.snapshot)?,
      branch: BranchName::new(&self.branch).map_err(|error| corrupt("run branch", error))?,
      base: Revision::new(&self.base).map_err(|error| corrupt("run base revision", error))?,
      base_branch: self
        .base_branch
        .as_deref()
        .map(BranchName::new)
        .transpose()
        .map_err(|error| corrupt("run base branch", error))?,
      base_source: super::super::base_branch::parse_source(source, module.as_deref())?,
      budget: optional_budget_value("run budget", initial_budget)?,
      max_duration: optional_duration_value("run time limit", initial_duration)?,
    };
    let stored = StoredRun {
      id,
      created,
      created_at: Timestamp::from_unix_millis(self.created_at),
      status: parse_run_status(
        &self.status,
        self.status_reason.as_deref(),
        (self.failed_step.as_deref(), self.limit_execution.as_deref()),
        self.failure_reason.as_deref(),
      )?,
      budget: optional_budget_value("run budget", budget)?,
      max_duration: optional_duration_value("run time limit", duration)?,
      step_budgets: records.step_budgets,
      permissions: records.permissions,
      used: TokensUsed::new(u64::try_from(self.used_tokens).map_err(|_| corrupt("run usage", self.used_tokens))?),
      pause_requested: self.pause_requested,
      attempts: records.attempts,
      last_checkpoint: records.last_checkpoint,
      last_sequence: sequence_value(self.last_sequence)?,
    };
    Run::restore(stored).map_err(|error| corrupt("run", error))
  }
}
