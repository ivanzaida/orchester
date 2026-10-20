//! Reading runs back. Rows are rebuilt through [`Run::restore`] and events through the domain
//! deserializers, so a damaged row surfaces as `StoreError::Corrupt`. Each run is read on its own:
//! a run whose record cannot be read back is listed as an [`UnreadableRun`] beside the others.

use std::{
  collections::BTreeMap,
  num::{NonZeroU32, NonZeroU64},
};

use orchester_core::{
  id::{ResourceId, RunId, TaskId},
  resource::StepKey,
  revision::Revision,
  run::{
    Attempt, BranchName, Checkpoint, EventSequence, ListedRun, Run, RunCreated, RunEvent, RunEventKind, RunPermissions,
    StepExecution, StoredRun, UnreadableRun,
  },
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};
use rusqlite::{Connection, OptionalExtension, Params, Row};

use super::{
  EVENT_SCHEMA, PERMISSIONS_SCHEMA, SNAPSHOT_SCHEMA,
  columns::{
    AttemptReasonColumns, parse_attempt_reason, parse_attempt_status, parse_run_status, parse_step_failure,
    parse_step_status, step_key,
  },
  legacy::{self, CreationLimits},
  run_number,
};
use crate::{
  RunStoreError, StoreError,
  rows::{corrupt, decode_body, decode_document, decode_json, optional_budget_value, optional_duration_value, parse},
};

const SELECT_RUNS: &str = "SELECT id, task_id, number, snapshot, branch, base_revision, initial_budget_tokens, \
                           initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens, pause_requested, \
                           status, status_reason, failed_step, limit_execution, last_sequence, created_at, \
                           base_branch, failure_reason FROM runs";

/// The runs `clause` selects, each read on its own. Only a run whose id or task cannot be read
/// fails the listing: it cannot be told which run it is.
///
/// A run is stored in several tables (its row, attempts, step executions, checkpoints, events),
/// read by separate statements in one [consistent read](super::super::consistent::read), so a run
/// row is never paired with rows written after it (the runner recording an attempt while the
/// desktop reads).
pub(super) fn load_runs(
  connection: &Connection,
  clause: &str,
  params: impl Params,
) -> Result<Vec<ListedRun<RunStoreError>>, StoreError> {
  super::super::consistent::read(connection, || read_runs(connection, clause, params))
}

fn read_runs(
  connection: &Connection,
  clause: &str,
  params: impl Params,
) -> Result<Vec<ListedRun<RunStoreError>>, StoreError> {
  let mut statement = connection.prepare(&format!("{SELECT_RUNS} {clause}"))?;
  let rows = statement
    .query_map(params, |row| Ok((RunIdentity::read(row)?, RunRow::read(row))))?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(identity, row)| {
      let id: RunId = parse("run id", &identity.id)?;
      let task: TaskId = parse("task id", &identity.task)?;
      let restored = row
        .map_err(StoreError::from)
        .and_then(|row| row.into_run(connection, (id, task, identity.number)));
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

fn load_step_budgets(connection: &Connection, run: &str) -> Result<BTreeMap<StepKey, Option<TokenBudget>>, StoreError> {
  let mut statement = connection.prepare("SELECT step_key, budget_tokens FROM run_step_budgets WHERE run_id = ?1")?;
  let rows = statement
    .query_map([run], |row| {
      Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(step, budget)| Ok((step_key(&step)?, optional_budget_value("step budget", budget)?)))
    .collect()
}

fn load_permissions(connection: &Connection, run: &str) -> Result<BTreeMap<ResourceId, RunPermissions>, StoreError> {
  let mut statement = connection.prepare("SELECT profile_id, body FROM run_permissions WHERE run_id = ?1")?;
  let rows = statement
    .query_map([run], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(profile, body)| {
      Ok((
        parse("profile id", &profile)?,
        decode_json("run permissions", PERMISSIONS_SCHEMA, &body)?,
      ))
    })
    .collect()
}

fn load_attempts(connection: &Connection, run: &str) -> Result<Vec<Attempt>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT id, number, reason, failed_step, return_to, note, status, conflicts FROM attempts WHERE run_id = ?1 \
     ORDER BY number",
  )?;
  type AttemptColumns = (
    String,
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
  );
  let rows = statement
    .query_map([run], |row| {
      Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
      ))
    })?
    .collect::<Result<Vec<AttemptColumns>, _>>()?;
  rows
    .into_iter()
    .map(
      |(id, number, reason, failed_step, return_to, note, status, conflicts)| {
        let number = u32::try_from(number)
          .ok()
          .and_then(NonZeroU32::new)
          .ok_or_else(|| corrupt("attempt number", number))?;
        Ok(Attempt::new(
          parse("attempt id", &id)?,
          number,
          parse_attempt_reason(&AttemptReasonColumns {
            reason: &reason,
            failed_step: failed_step.as_deref(),
            return_to: return_to.as_deref(),
            note: note.as_deref(),
            conflicts: conflicts.as_deref(),
          })?,
          parse_attempt_status(&status)?,
          load_executions(connection, &id)?,
        ))
      },
    )
    .collect()
}

/// Executions in insertion order, which is pipeline order within an attempt.
fn load_executions(connection: &Connection, attempt: &str) -> Result<Vec<StepExecution>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT id, step_key, status, wait_reason, failure, judged_revision FROM step_executions WHERE attempt_id = ?1 \
     ORDER BY rowid",
  )?;
  let rows = statement
    .query_map([attempt], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, Option<String>>(3)?,
        row.get::<_, Option<String>>(4)?,
        row.get::<_, Option<String>>(5)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(id, step, status, wait_reason, failure, judged)| {
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
    })
    .collect()
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
    })
  }

  /// The run `id` of `task`, numbered `number`, with these columns and its other rows.
  fn into_run(self, connection: &Connection, (id, task, number): (RunId, TaskId, i64)) -> Result<Run, StoreError> {
    let row = id.to_string();
    let [initial_budget, initial_duration, budget, duration] = self.limits;
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
      step_budgets: load_step_budgets(connection, &row)?,
      permissions: load_permissions(connection, &row)?,
      used: TokensUsed::new(u64::try_from(self.used_tokens).map_err(|_| corrupt("run usage", self.used_tokens))?),
      pause_requested: self.pause_requested,
      attempts: load_attempts(connection, &row)?,
      last_checkpoint: load_checkpoint(connection, id)?,
      last_sequence: sequence_value(self.last_sequence)?,
    };
    Run::restore(stored).map_err(|error| corrupt("run", error))
  }
}
