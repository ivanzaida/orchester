//! What merged tasks took (`task_effort`), beside the points they had when they merged: the calibration of story
//! points ([`orchester_core::task::estimate::calibration`]).

use orchester_core::{
  connection::stream::AgentEvent,
  id::TaskId,
  run::TokenUsage,
  task::estimate::calibration::{CalibrationRecord, TaskEffort},
  time::Timestamp,
};
use rusqlite::{Transaction, params};

use super::{
  ProjectDb,
  activity::EVENT_SCHEMA,
  estimates::{self, parse_source, points_value, source_text},
};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, parse, tokens_column},
};

/// The runs of the task `?1`.
const TASK_RUNS: &str = "SELECT id FROM runs WHERE task_id = ?1";

impl ProjectDb {
  /// The project's merged tasks with what they took and the points they had then, latest merged first.
  pub fn calibration_records(&self) -> Result<Vec<CalibrationRecord>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT effort.task_id, tasks.number, tasks.title, tasks.brief, effort.points, effort.source, effort.tokens, \
       effort.active_ms, effort.attempts, effort.runs, effort.merged_at \
       FROM task_effort AS effort JOIN tasks ON tasks.id = effort.task_id ORDER BY effort.merged_at DESC",
    )?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          (
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
          ),
          (row.get::<_, Option<i64>>(4)?, row.get::<_, Option<String>>(5)?),
          [row.get::<_, i64>(6)?, row.get(7)?, row.get(8)?, row.get(9)?],
          row.get::<_, i64>(10)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    let key = self.project().key().as_str().to_owned();
    rows
      .into_iter()
      .map(
        |((task, number, title, brief), points, [tokens, active_ms, attempts, runs], merged_at)| {
          let count = |value: i64| u64::try_from(value).map_err(|_| corrupt("task effort", value));
          let few = |value: i64| u32::try_from(value).map_err(|_| corrupt("task effort", value));
          let points = match points {
            (Some(points), Some(source)) => Some((points_value(points)?, parse_source(&source)?)),
            (None, None) => None,
            partly => return Err(corrupt("task effort points", format!("{partly:?}"))),
          };
          Ok(CalibrationRecord {
            task: parse("task id", &task)?,
            key: format!("{key}-{number}"),
            title,
            brief,
            points,
            effort: TaskEffort {
              tokens: count(tokens)?,
              active_ms: count(active_ms)?,
              attempts: few(attempts)?,
              runs: few(runs)?,
            },
            merged_at: Timestamp::from_unix_millis(merged_at),
          })
        },
      )
      .collect()
  }
}

/// Keeps what `task` took over all its runs beside the points it has now, as merged at `merged_at`, in the
/// transaction that records the merge.
pub(super) fn record(transaction: &Transaction<'_>, task: TaskId, merged_at: Timestamp) -> Result<(), StoreError> {
  let effort = measure(transaction, task)?;
  let estimate = estimates::load(transaction, task)?;
  transaction.execute(
    "INSERT INTO task_effort (task_id, merged_at, points, source, tokens, active_ms, attempts, runs) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
     ON CONFLICT (task_id) DO UPDATE SET merged_at = ?2, points = ?3, source = ?4, tokens = ?5, active_ms = ?6, \
     attempts = ?7, runs = ?8",
    params![
      task.to_string(),
      merged_at.unix_millis(),
      estimate.as_ref().map(|estimate| estimate.points().get()),
      estimate.as_ref().map(|estimate| source_text(estimate.source())),
      tokens_column("task effort tokens", effort.tokens)?,
      tokens_column("task effort time", effort.active_ms)?,
      effort.attempts,
      effort.runs,
    ],
  )?;
  Ok(())
}

/// What `task`'s runs took: the tokens of its agents' turns and of auto mode's reviews of their requests, the
/// active time of its agents (waits for a person left out), its attempts and its runs.
fn measure(transaction: &Transaction<'_>, task: TaskId) -> Result<TaskEffort, StoreError> {
  let id = task.to_string();
  let mut statement = transaction.prepare(&format!(
    "SELECT payload FROM agent_events WHERE kind = 'turn_completed' AND run_id IN ({TASK_RUNS})"
  ))?;
  let payloads = statement
    .query_map([&id], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  let mut tokens = 0_u64;
  for payload in payloads {
    match decode_json("agent event", EVENT_SCHEMA, &payload)? {
      AgentEvent::TurnCompleted { usage } => tokens = tokens.saturating_add(usage.counted()),
      other => {
        return Err(corrupt(
          "agent event",
          format!("a `turn_completed` row holds `{}`", other.name()),
        ));
      }
    }
  }
  let reviewed: [i64; 3] = transaction.query_row(
    &format!(
      "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(cached_input_tokens), 0), COALESCE(SUM(output_tokens), 0) \
       FROM reviewer_usage WHERE input_tokens IS NOT NULL AND run_id IN ({TASK_RUNS})"
    ),
    [&id],
    |row| Ok([row.get(0)?, row.get(1)?, row.get(2)?]),
  )?;
  let count = |value: i64| u64::try_from(value).map_err(|_| corrupt("task effort", value));
  let reviewed = TokenUsage {
    input: count(reviewed[0])?,
    cached_input: count(reviewed[1])?,
    output: count(reviewed[2])?,
  };
  let (active_ms, attempts, runs): (i64, i64, i64) = transaction.query_row(
    &format!(
      "SELECT \
         (SELECT COALESCE(SUM(MAX(0, ended_at - started_at - inactive_ms)), 0) FROM invocations \
          WHERE kind = 'agent' AND ended_at IS NOT NULL AND run_id IN ({TASK_RUNS})), \
         (SELECT COUNT(*) FROM attempts WHERE run_id IN ({TASK_RUNS})), \
         (SELECT COUNT(*) FROM runs WHERE task_id = ?1)"
    ),
    [&id],
    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
  )?;
  let few = |value: i64| u32::try_from(value).map_err(|_| corrupt("task effort", value));
  Ok(TaskEffort {
    tokens: tokens.saturating_add(reviewed.counted()),
    active_ms: count(active_ms)?,
    attempts: few(attempts)?,
    runs: few(runs)?,
  })
}
