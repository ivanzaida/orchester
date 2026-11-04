//! Fix CI on a run (`run_ci_fix_settings`, `run_ci_fix_events`, ORC-335): the desktop writes the run's *Fix CI
//! automatically* switch as the person sets it, and each Fix CI it sent, by hand or automatically, or stopped at; the
//! task page reads them with the run.

use orchester_core::{
  id::RunId,
  pull_request::ci::fix::{AutoFix, FixEvent, FixEventKind, FixLimit, FixedBy, RunFixes},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// A `run_ci_fix_events` row as stored, before its columns are read back.
type EventRow = (
  i64,
  i64,
  String,
  String,
  String,
  Option<String>,
  Option<i64>,
  Option<i64>,
);

impl ProjectDb {
  /// Sets `run`'s *Fix CI automatically* switch as the person set it at `at`, which restarts its count.
  pub fn set_auto_fix(&mut self, run: RunId, auto: AutoFix, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO run_ci_fix_settings (run_id, automatic, fix_limit, changed_at) VALUES (?1, ?2, ?3, ?4) \
       ON CONFLICT (run_id) DO UPDATE SET automatic = excluded.automatic, fix_limit = excluded.fix_limit, \
       changed_at = excluded.changed_at",
      params![
        run.to_string(),
        i64::from(auto.on),
        i64::from(auto.limit.get()),
        at.unix_millis()
      ],
    )?;
    Ok(())
  }

  /// Appends `event` to `run`'s Fix CI events.
  pub fn record_ci_fix(&mut self, run: RunId, event: &FixEvent) -> Result<(), StoreError> {
    let failing = serde_json::to_string(&event.failing).map_err(|_| StoreError::Unrepresentable("failing checks"))?;
    let number = i64::try_from(event.number).map_err(|_| StoreError::Unrepresentable("pull request number"))?;
    let (kind, message, count, limit) = match event.kind {
      FixEventKind::Sent { message, by } => {
        let (count, limit) = match by {
          FixedBy::Hand => (None, None),
          FixedBy::Automatically { count, limit } => (Some(i64::from(count)), Some(i64::from(limit.get()))),
        };
        ("sent", Some(message.to_string()), count, limit)
      }
      FixEventKind::Stopped { limit } => ("stopped", None, None, Some(i64::from(limit.get()))),
    };
    self.connection.execute(
      "INSERT INTO run_ci_fix_events (run_id, at, pull_request, failure, failing, kind, message_id, fix_count, \
       fix_limit) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
      params![
        run.to_string(),
        event.at.unix_millis(),
        number,
        event.failure,
        failing,
        kind,
        message,
        count,
        limit
      ],
    )?;
    Ok(())
  }

  /// `run`'s Fix CI: its switch (off with the default limit while never set) and its events, oldest first.
  pub fn ci_fixes(&self, run: RunId) -> Result<RunFixes, StoreError> {
    let settings = self
      .connection
      .query_row(
        "SELECT automatic, fix_limit, changed_at FROM run_ci_fix_settings WHERE run_id = ?1",
        [run.to_string()],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
      )
      .optional()?;
    let (auto, changed_at) = match settings {
      Some((on, limit, at)) => (
        AutoFix {
          on: on == 1,
          limit: limit_of(limit)?,
        },
        Some(Timestamp::from_unix_millis(at)),
      ),
      None => (AutoFix::default(), None),
    };
    let mut query = self.connection.prepare(
      "SELECT at, pull_request, failure, failing, kind, message_id, fix_count, fix_limit FROM run_ci_fix_events \
       WHERE run_id = ?1 ORDER BY id",
    )?;
    let rows: Vec<EventRow> = query
      .query_map([run.to_string()], |row| {
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
      .collect::<Result<_, _>>()?;
    let events = rows.into_iter().map(event).collect::<Result<_, _>>()?;
    Ok(RunFixes {
      auto,
      changed_at,
      events,
    })
  }

  /// The runs whose *Fix CI automatically* is on, each the latest run of its task.
  pub fn runs_fixing_ci_automatically(&self) -> Result<Vec<RunId>, StoreError> {
    let mut query = self.connection.prepare(
      "SELECT settings.run_id FROM run_ci_fix_settings AS settings JOIN runs ON runs.id = settings.run_id \
       WHERE settings.automatic = 1 \
         AND runs.number = (SELECT MAX(later.number) FROM runs AS later WHERE later.task_id = runs.task_id) \
       ORDER BY runs.number",
    )?;
    let ids = query
      .query_map([], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    ids.iter().map(|id| parse("run id", id)).collect()
  }
}

fn limit_of(count: i64) -> Result<FixLimit, StoreError> {
  u32::try_from(count)
    .ok()
    .and_then(FixLimit::of)
    .ok_or_else(|| corrupt("Fix CI limit", count))
}

fn event((at, number, failure, failing, kind, message, count, limit): EventRow) -> Result<FixEvent, StoreError> {
  let kind = match (kind.as_str(), message, count, limit) {
    ("sent", Some(message), None, None) => FixEventKind::Sent {
      message: parse("Fix CI message id", &message)?,
      by: FixedBy::Hand,
    },
    ("sent", Some(message), Some(count), Some(limit)) => FixEventKind::Sent {
      message: parse("Fix CI message id", &message)?,
      by: FixedBy::Automatically {
        count: u32::try_from(count).map_err(|_| corrupt("Fix CI count", count))?,
        limit: limit_of(limit)?,
      },
    },
    ("stopped", None, None, Some(limit)) => FixEventKind::Stopped {
      limit: limit_of(limit)?,
    },
    (other, ..) => return Err(corrupt("Fix CI event", other)),
  };
  Ok(FixEvent {
    at: Timestamp::from_unix_millis(at),
    number: u64::try_from(number).map_err(|_| corrupt("Fix CI pull request", number))?,
    failure,
    failing: serde_json::from_str(&failing).map_err(|error| corrupt("Fix CI failing checks", error))?,
    kind,
  })
}
