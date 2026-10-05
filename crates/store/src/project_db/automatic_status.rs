//! The workflow status a run of a shared task moved it to (`docs/architecture/server.md`, "Claims"):
//! the server has a shared task's organization, so the run's change is recorded here with the
//! status it changed from, and the holder's desktop sends it as Orchester's change only while the
//! server still has that status and the person still holds the task. A status that was not sent says
//! why on the task until the person dismisses it.

use orchester_core::{id::TaskId, task::organization::WorkflowStatus, time::Timestamp};
use rusqlite::{Connection, OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

/// A run's workflow status waiting to be sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutomaticStatus {
  pub task: TaskId,
  /// The status the task had when the run changed it, as the cache had it.
  pub from: WorkflowStatus,
  pub status: WorkflowStatus,
  pub at: Timestamp,
}

/// Why a run's workflow status was not sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotSent {
  /// The task's status on the server changed meanwhile, to `current`; the server's stays.
  Changed { current: WorkflowStatus },
  /// The person no longer holds the task.
  NotHolder,
}

/// A run's workflow status that was not sent, shown on the task until dismissed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsentStatus {
  pub status: WorkflowStatus,
  pub reason: NotSent,
  pub at: Timestamp,
}

impl ProjectDb {
  /// The runs' workflow statuses the server does not have yet, oldest first.
  pub fn automatic_statuses(&self) -> Result<Vec<AutomaticStatus>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT task_id, from_status, status, changed_at FROM shared_automatic_status \
       WHERE not_sent IS NULL ORDER BY changed_at",
    )?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, i64>(3)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(task, from, status, at)| {
        Ok(AutomaticStatus {
          task: parse("task id", &task)?,
          from: decode_json("automatic status", 1, &from)?,
          status: decode_json("automatic status", 1, &status)?,
          at: Timestamp::from_unix_millis(at),
        })
      })
      .collect()
  }

  /// Forgets the status of `task` recorded at `at`, once the server has it; a later one stays.
  pub fn clear_automatic_status(&mut self, task: TaskId, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "DELETE FROM shared_automatic_status WHERE task_id = ?1 AND changed_at = ?2",
      params![task.to_string(), at.unix_millis()],
    )?;
    Ok(())
  }

  /// Records that the status of `task` recorded at `at` is not sent, and why; a later one stays to
  /// be sent.
  pub fn record_not_sent(&mut self, task: TaskId, at: Timestamp, reason: NotSent) -> Result<(), StoreError> {
    let (kind, current) = match reason {
      NotSent::Changed { current } => ("changed", Some(encode_json("automatic status", 1, &current)?)),
      NotSent::NotHolder => ("not_holder", None),
    };
    self.connection.execute(
      "UPDATE shared_automatic_status SET not_sent = ?3, server_status = ?4 \
       WHERE task_id = ?1 AND changed_at = ?2",
      params![task.to_string(), at.unix_millis(), kind, current],
    )?;
    Ok(())
  }

  /// The run's status of `task` that was not sent, if the person has not dismissed it.
  pub fn unsent_status(&self, task: TaskId) -> Result<Option<UnsentStatus>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT status, not_sent, server_status, changed_at FROM shared_automatic_status \
         WHERE task_id = ?1 AND not_sent IS NOT NULL",
        [task.to_string()],
        |row| {
          Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)?,
          ))
        },
      )
      .optional()?;
    let Some((status, kind, current, at)) = row else {
      return Ok(None);
    };
    let reason = match (kind.as_str(), current) {
      ("changed", Some(current)) => NotSent::Changed {
        current: decode_json("automatic status", 1, &current)?,
      },
      ("not_holder", None) => NotSent::NotHolder,
      _ => return Err(corrupt("unsent automatic status", kind)),
    };
    Ok(Some(UnsentStatus {
      status: decode_json("automatic status", 1, &status)?,
      reason,
      at: Timestamp::from_unix_millis(at),
    }))
  }

  /// The person read why the run's status of `task` was not sent.
  pub fn dismiss_unsent_status(&mut self, task: TaskId) -> Result<(), StoreError> {
    self.connection.execute(
      "DELETE FROM shared_automatic_status WHERE task_id = ?1 AND not_sent IS NOT NULL",
      [task.to_string()],
    )?;
    Ok(())
  }
}

/// Records that a run moved shared `task` from `from` to `status`. A status still waiting keeps the
/// status it changed from: that is what the server must still have for it to be sent.
pub(super) fn record(
  connection: &Connection,
  task: TaskId,
  (from, status): (WorkflowStatus, WorkflowStatus),
  at: Timestamp,
) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO shared_automatic_status (task_id, from_status, status, changed_at) VALUES (?1, ?2, ?3, ?4) \
     ON CONFLICT (task_id) DO UPDATE SET status = excluded.status, changed_at = excluded.changed_at, \
     from_status = CASE WHEN not_sent IS NULL THEN from_status ELSE excluded.from_status END, \
     not_sent = NULL, server_status = NULL",
    params![
      task.to_string(),
      encode_json("automatic status", 1, &from)?,
      encode_json("automatic status", 1, &status)?,
      at.unix_millis()
    ],
  )?;
  Ok(())
}
