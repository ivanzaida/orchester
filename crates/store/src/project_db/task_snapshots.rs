//! The kept versions of a task's requirements: as the task was created (`task_creation_snapshots`) and as
//! a run was given it when it started (`run_task_snapshots`, written by `tasks`). Both are immutable once
//! written, and a version that was not kept reads as none: never as the task's current content.

use orchester_core::{
  id::{RunId, TaskId},
  name::Name,
  sharing::TaskRevision,
  task::{Task, TaskDetails},
};
use rusqlite::{OptionalExtension, Row, Transaction, params};

use super::{ProjectDb, shared_tasks::CachedTask, tasks};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

/// A kept version as stored.
struct Kept {
  title: String,
  brief: String,
  criteria: String,
  pipeline: Option<String>,
  budget: Option<i64>,
  duration: Option<i64>,
}

impl Kept {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      title: row.get(0)?,
      brief: row.get(1)?,
      criteria: row.get(2)?,
      pipeline: row.get(3)?,
      budget: row.get(4)?,
      duration: row.get(5)?,
    })
  }

  fn into_details(self, what: &'static str) -> Result<TaskDetails, StoreError> {
    Ok(TaskDetails {
      title: Name::new(&self.title).map_err(|error| corrupt(what, error))?,
      brief: self.brief,
      acceptance_criteria: decode_json(what, 1, &self.criteria)?,
      pipeline: self.pipeline.as_deref().map(|id| parse(what, id)).transpose()?,
      limits: tasks::task_limits(self.budget, self.duration)?,
    })
  }
}

impl ProjectDb {
  /// What `task` asked for when it was created; `None` for a task created before creation versions were
  /// kept, or a shared task this computer did not create. A shared task's pipeline is this computer's
  /// pipeline of its slug, as for the task itself.
  pub fn task_creation_details(&self, task: TaskId) -> Result<Option<TaskDetails>, StoreError> {
    let kept = self
      .connection
      .query_row(
        "SELECT title, brief, criteria, COALESCE(pipeline_id, (SELECT resources.id FROM resources \
         WHERE resources.kind = 'pipeline' AND resources.slug = task_creation_snapshots.pipeline_slug)), \
         budget_tokens, max_duration_ms FROM task_creation_snapshots WHERE task_id = ?1",
        [task.to_string()],
        Kept::read,
      )
      .optional()?;
    kept.map(|kept| kept.into_details("task creation snapshot")).transpose()
  }

  /// What `run` was given when it started; `None` when it was not kept (a run from before run snapshots,
  /// whose task was not edited since).
  pub fn run_start_details(&self, run: RunId) -> Result<Option<TaskDetails>, StoreError> {
    let kept = self
      .connection
      .query_row(
        "SELECT title, brief, criteria, pipeline_id, budget_tokens, max_duration_ms FROM run_task_snapshots \
         WHERE run_id = ?1",
        [run.to_string()],
        Kept::read,
      )
      .optional()?;
    kept.map(|kept| kept.into_details("run task snapshot")).transpose()
  }
}

/// Keeps what `task`, created now, asks for, inside its creation transaction.
pub(super) fn record_creation(transaction: &Transaction<'_>, task: &Task) -> Result<(), StoreError> {
  let details = task.details();
  let (budget, duration) = tasks::limit_columns(details.limits)?;
  transaction.execute(
    "INSERT INTO task_creation_snapshots (task_id, title, brief, criteria, pipeline_id, budget_tokens, \
     max_duration_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![
      task.id().to_string(),
      details.title.as_str(),
      details.brief,
      encode_json("task creation criteria", 1, &details.acceptance_criteria)?,
      details.pipeline.map(|id| id.to_string()),
      budget,
      duration
    ],
  )?;
  Ok(())
}

/// Keeps what a shared task this computer just created asks for, as the server confirmed it: only at its
/// first revision, so a task someone changed before this computer read it is not taken for its creation.
pub(super) fn record_shared_creation(transaction: &Transaction<'_>, cached: &CachedTask) -> Result<(), StoreError> {
  if cached.revision != TaskRevision::FIRST {
    return Ok(());
  }
  let (budget, duration) = tasks::limit_columns(cached.limits)?;
  transaction.execute(
    "INSERT INTO task_creation_snapshots (task_id, title, brief, criteria, pipeline_slug, budget_tokens, \
     max_duration_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![
      cached.id.to_string(),
      cached.title.as_str(),
      cached.brief,
      encode_json("task creation criteria", 1, &cached.criteria)?,
      cached.pipeline,
      budget,
      duration
    ],
  )?;
  Ok(())
}
