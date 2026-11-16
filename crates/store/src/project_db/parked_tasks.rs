//! The tasks the Orchestrator parked for the person (`orchester_core::orchestrator::parking`): kept with where the
//! task's run stood, so that a run that moved since, or another run of the task, tells that someone acted on it.

use orchester_core::{
  id::{RunId, TaskId},
  orchestrator::parking::{Needs, Parking, Words},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{StoreError, rows::parse};

const COLUMNS: &str = "task_id, run_id, needs, why, to_do, chat_id, at";

impl ProjectDb {
  /// Parks `parking`'s task, in place of an earlier parking of it, with the status its run has now.
  pub fn park_task(&mut self, parking: &Parking) -> Result<(), StoreError> {
    let run = parking.run.map(|run| run.to_string());
    self.connection.execute(
      "INSERT INTO parked_tasks (task_id, run_id, run_status, needs, why, to_do, chat_id, at)
       VALUES (?1, ?2, (SELECT status FROM runs WHERE id = ?2), ?3, ?4, ?5, ?6, ?7)
       ON CONFLICT (task_id) DO UPDATE SET run_id = excluded.run_id, run_status = excluded.run_status,
         needs = excluded.needs, why = excluded.why, to_do = excluded.to_do, chat_id = excluded.chat_id,
         at = excluded.at",
      params![
        parking.task.to_string(),
        run,
        parking.needs.name(),
        parking.why.as_str(),
        parking.to_do.as_str(),
        parking.by.to_string(),
        parking.at.unix_millis(),
      ],
    )?;
    Ok(())
  }

  /// Every parked task, the one parked first first.
  pub fn parked_tasks(&self) -> Result<Vec<Parking>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("SELECT {COLUMNS} FROM parked_tasks ORDER BY at, task_id"))?;
    let rows = statement.query_map([], stored)?.collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(Stored::parking).collect()
  }

  /// The parking of `task`; `None` while it is not parked.
  pub fn parked_task(&self, task: TaskId) -> Result<Option<Parking>, StoreError> {
    let row = self
      .connection
      .query_row(
        &format!("SELECT {COLUMNS} FROM parked_tasks WHERE task_id = ?1"),
        [task.to_string()],
        stored,
      )
      .optional()?;
    row.map(Stored::parking).transpose()
  }

  /// Ends the parking of `task`; the parking it had, `None` when it was not parked.
  pub fn unpark_task(&mut self, task: TaskId) -> Result<Option<Parking>, StoreError> {
    let transaction = self.connection.transaction()?;
    let row = transaction
      .query_row(
        &format!("SELECT {COLUMNS} FROM parked_tasks WHERE task_id = ?1"),
        [task.to_string()],
        stored,
      )
      .optional()?;
    transaction.execute("DELETE FROM parked_tasks WHERE task_id = ?1", [task.to_string()])?;
    transaction.commit()?;
    row.map(Stored::parking).transpose()
  }

  /// Makes the waits and the run ends kept of `run` that have no turn yet, or whose turn failed, start none by
  /// themselves any more: its task was left to the person, and nothing is tried again for it. How many it settled.
  pub fn settle_waits_of_run(&mut self, run: RunId) -> Result<usize, StoreError> {
    Ok(self.connection.execute(
      "UPDATE orchestrator_entries SET body = json_remove(body, '$.automatic')
       WHERE kind = 'wait' AND json_extract(body, '$.run') = ?1 AND json_extract(body, '$.automatic') = 1
         AND (message_id IS NULL OR turn = 'failed')",
      [run.to_string()],
    )?)
  }

  /// The parked tasks someone acted on since: the task's latest run is not the one it was parked with (it had none
  /// and has one now, or has a later one), or that run's status changed.
  pub fn parkings_moved(&self) -> Result<Vec<TaskId>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT p.task_id FROM parked_tasks p
       WHERE (SELECT r.id FROM runs r WHERE r.task_id = p.task_id ORDER BY r.number DESC LIMIT 1) IS NOT p.run_id
          OR (SELECT r.status FROM runs r WHERE r.id = p.run_id) IS NOT p.run_status
       ORDER BY p.at, p.task_id",
    )?;
    let rows = statement
      .query_map([], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    rows.iter().map(|task| parse("task id", task)).collect()
  }
}

/// A row as it is stored, before its ids and names are read.
struct Stored {
  task: String,
  run: Option<String>,
  needs: String,
  why: String,
  to_do: String,
  chat: String,
  at: i64,
}

fn stored(row: &Row<'_>) -> rusqlite::Result<Stored> {
  Ok(Stored {
    task: row.get(0)?,
    run: row.get(1)?,
    needs: row.get(2)?,
    why: row.get(3)?,
    to_do: row.get(4)?,
    chat: row.get(5)?,
    at: row.get(6)?,
  })
}

impl Stored {
  fn parking(self) -> Result<Parking, StoreError> {
    let words = |what: &'static str, text: &str| {
      Words::new(text).ok_or_else(|| StoreError::Corrupt {
        what,
        detail: text.to_owned(),
      })
    };
    Ok(Parking {
      task: parse("task id", &self.task)?,
      run: self.run.as_deref().map(|run| parse("run id", run)).transpose()?,
      // A kind this version does not know reads as something else the person is to do: the words say what.
      needs: Needs::named(&self.needs).unwrap_or(Needs::Other),
      why: words("why a task is parked", &self.why)?,
      to_do: words("what a parked task needs", &self.to_do)?,
      by: parse("chat id", &self.chat)?,
      at: Timestamp::from_unix_millis(self.at),
    })
  }
}
