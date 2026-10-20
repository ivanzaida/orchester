//! The ends of the project's runs the Orchestrator chat follows (`orchester_core::orchestrator::RunEnd`): since when
//! it follows them, and the tasks' latest runs that ended or stopped since, each by the event that put it there,
//! which keeps each end once. Read in one query, without loading the runs.

use orchester_core::{id::RunId, run::EventSequence, time::Timestamp};
use rusqlite::params;

use super::{ProjectDb, runs::sequence_value};
use crate::{StoreError, rows::parse};

/// A task's latest run whose latest event that ends or stops a run is at or after since when the chat follows ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndedRun {
  pub run: RunId,
  /// That event: its sequence, time and kind (`RunEventKind::name`).
  pub sequence: EventSequence,
  pub at: Timestamp,
  pub kind: String,
  /// The kind of the event just before it, when there is one: a person's decision on a stopped run comes right
  /// before the failure it ends in.
  pub after: Option<String>,
}

impl ProjectDb {
  /// Since when the chat follows the ends of the project's runs: an end before it is not followed.
  pub fn run_ends_since(&self) -> Result<Timestamp, StoreError> {
    let since = self.connection.query_row(
      "SELECT run_ends_since FROM orchestrator_follow WHERE id = 1",
      [],
      |row| row.get::<_, i64>(0),
    )?;
    Ok(Timestamp::from_unix_millis(since))
  }

  /// The tasks' latest runs whose latest event that ends or stops a run (it succeeded, failed, waits for a person,
  /// paused or was interrupted) is at `since` or later; whether the run still stands there is the caller's to read.
  pub fn ended_runs_since(&self, since: Timestamp) -> Result<Vec<EndedRun>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT r.id, e.sequence, e.at, e.kind,
         (SELECT p.kind FROM run_events p WHERE p.run_id = r.id AND p.sequence = e.sequence - 1)
       FROM runs r
       JOIN run_events e ON e.run_id = r.id AND e.sequence = (
         SELECT x.sequence FROM run_events x
         WHERE x.run_id = r.id
           AND x.kind IN ('succeeded', 'failed', 'waiting_for_person', 'paused', 'interrupted')
         ORDER BY x.sequence DESC LIMIT 1)
       WHERE e.at >= ?1 AND NOT EXISTS (SELECT 1 FROM runs n WHERE n.task_id = r.task_id AND n.number > r.number)
       ORDER BY e.at, r.number",
    )?;
    let rows = statement
      .query_map(params![since.unix_millis()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, i64>(1)?,
          row.get::<_, i64>(2)?,
          row.get::<_, String>(3)?,
          row.get::<_, Option<String>>(4)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(run, sequence, at, kind, after)| {
        Ok(EndedRun {
          run: parse("run id", &run)?,
          sequence: sequence_value(sequence)?,
          at: Timestamp::from_unix_millis(at),
          kind,
          after,
        })
      })
      .collect()
  }

  /// When `run` was interrupted, oldest first: a run interrupted again soon after is not continued by the
  /// Orchestrator in a loop (`orchester_core::orchestrator::Continuing`).
  pub fn run_interruptions(&self, run: RunId) -> Result<Vec<Timestamp>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT at FROM run_events WHERE run_id = ?1 AND kind = 'interrupted' ORDER BY sequence")?;
    let times = statement
      .query_map(params![run.to_string()], |row| row.get::<_, i64>(0))?
      .map(|at| at.map(Timestamp::from_unix_millis))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(times)
  }
}
