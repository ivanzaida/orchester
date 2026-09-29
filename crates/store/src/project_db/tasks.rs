use std::num::NonZeroU32;

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  name::Name,
  project::{ProjectKey, TaskKey},
  store::TaskStore,
  task::{Task, TaskDetails, TaskLimits, TaskStatus},
  time::Timestamp,
};
use rusqlite::{Connection, Params, Row, Transaction, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{budget_value, corrupt, duration_value, millis_column, parse, tokens_column},
};

const SELECT_TASKS: &str =
  "SELECT id, number, title, brief, pipeline_id, budget_tokens, max_duration_ms, status, created_at FROM tasks";

impl TaskStore for ProjectDb {
  type Error = StoreError;

  fn create_tasks(
    &mut self,
    batch: IdempotencyKey,
    details: Vec<TaskDetails>,
    now: Timestamp,
  ) -> Result<Vec<Task>, StoreError> {
    if details.is_empty() {
      return Err(StoreError::EmptyBatch);
    }
    let project_key = self.project.key().clone();
    let batch = batch.to_string();
    let transaction = self.connection.transaction()?;
    let existing = load_tasks(
      &transaction,
      &project_key,
      "WHERE batch_key = ?1 ORDER BY number",
      [&batch],
    )?;
    if !existing.is_empty() {
      return Ok(existing);
    }
    let first: i64 = transaction.query_row("SELECT next FROM counters WHERE name = 'task'", [], |row| row.get(0))?;
    let mut tasks = Vec::with_capacity(details.len());
    for (number, details) in (first..).zip(details) {
      let key = TaskKey::new(project_key.clone(), task_number(number)?);
      let task = Task::draft(TaskId::generate(), key, details, now);
      insert_task(&transaction, &task, &batch)?;
      tasks.push(task);
    }
    let next = first + i64::try_from(tasks.len()).map_err(|_| StoreError::Unrepresentable("task number"))?;
    transaction.execute("UPDATE counters SET next = ?1 WHERE name = 'task'", [next])?;
    transaction.commit()?;
    Ok(tasks)
  }

  fn task(&self, id: TaskId) -> Result<Option<Task>, StoreError> {
    let mut found = load_tasks(&self.connection, self.project.key(), "WHERE id = ?1", [id.to_string()])?;
    Ok(found.pop())
  }

  fn tasks(&self) -> Result<Vec<Task>, StoreError> {
    load_tasks(&self.connection, self.project.key(), "ORDER BY number", [])
  }

  fn save_task(&mut self, task: &Task) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    update_task(&transaction, task)?;
    transaction.commit()?;
    Ok(())
  }

  fn update_draft(&mut self, id: TaskId, details: TaskDetails) -> Result<Task, StoreError> {
    let transaction = self.connection.transaction()?;
    let mut task = load_tasks(&transaction, self.project.key(), "WHERE id = ?1", [id.to_string()])?
      .pop()
      .ok_or(StoreError::TaskNotFound(id))?;
    let has_run: bool = transaction.query_row(
      "SELECT EXISTS (SELECT 1 FROM runs WHERE task_id = ?1)",
      [id.to_string()],
      |row| row.get(0),
    )?;
    task
      .edit(details, has_run)
      .map_err(|source| StoreError::TaskNotEditable { task: id, source })?;
    update_task(&transaction, &task)?;
    transaction.commit()?;
    Ok(task)
  }
}

/// Stores the details and status of `task`, which must exist.
fn update_task(transaction: &Transaction<'_>, task: &Task) -> Result<(), StoreError> {
  let details = task.details();
  let (budget_tokens, max_duration_ms) = limit_columns(details.limits)?;
  let updated = transaction.execute(
    "UPDATE tasks SET title = ?2, brief = ?3, pipeline_id = ?4, budget_tokens = ?5, max_duration_ms = ?6, \
     status = ?7 WHERE id = ?1",
    params![
      task.id().to_string(),
      details.title.as_str(),
      details.brief,
      details.pipeline.map(|id| id.to_string()),
      budget_tokens,
      max_duration_ms,
      status_text(task.status()),
    ],
  )?;
  if updated == 0 {
    return Err(StoreError::TaskNotFound(task.id()));
  }
  write_criteria(transaction, task.id(), &details.acceptance_criteria)
}

fn insert_task(transaction: &Transaction<'_>, task: &Task, batch: &str) -> Result<(), StoreError> {
  let details = task.details();
  let (budget_tokens, max_duration_ms) = limit_columns(details.limits)?;
  transaction.execute(
    "INSERT INTO tasks (id, number, batch_key, title, brief, pipeline_id, budget_tokens, max_duration_ms, status, \
     created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    params![
      task.id().to_string(),
      task.key().number().get(),
      batch,
      details.title.as_str(),
      details.brief,
      details.pipeline.map(|id| id.to_string()),
      budget_tokens,
      max_duration_ms,
      status_text(task.status()),
      task.created_at().unix_millis(),
    ],
  )?;
  write_criteria(transaction, task.id(), &details.acceptance_criteria)
}

fn write_criteria(transaction: &Transaction<'_>, task: TaskId, criteria: &[String]) -> Result<(), StoreError> {
  let task = task.to_string();
  transaction.execute("DELETE FROM task_criteria WHERE task_id = ?1", [&task])?;
  let mut insert = transaction.prepare("INSERT INTO task_criteria (task_id, position, text) VALUES (?1, ?2, ?3)")?;
  for (position, criterion) in criteria.iter().enumerate() {
    let position = i64::try_from(position).map_err(|_| StoreError::Unrepresentable("criterion position"))?;
    insert.execute(params![task, position, criterion])?;
  }
  Ok(())
}

fn load_tasks(
  connection: &Connection,
  project: &ProjectKey,
  clause: &str,
  params: impl Params,
) -> Result<Vec<Task>, StoreError> {
  let mut statement = connection.prepare(&format!("{SELECT_TASKS} {clause}"))?;
  let rows = statement
    .query_map(params, TaskRow::read)?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|row| {
      let criteria = load_criteria(connection, &row.id)?;
      row.into_task(project, criteria)
    })
    .collect()
}

fn load_criteria(connection: &Connection, task: &str) -> Result<Vec<String>, StoreError> {
  let mut statement = connection.prepare("SELECT text FROM task_criteria WHERE task_id = ?1 ORDER BY position")?;
  let criteria = statement
    .query_map([task], |row| row.get(0))?
    .collect::<Result<Vec<String>, _>>()?;
  Ok(criteria)
}

struct TaskRow {
  id: String,
  number: i64,
  title: String,
  brief: String,
  pipeline: Option<String>,
  budget_tokens: Option<i64>,
  max_duration_ms: Option<i64>,
  status: String,
  created_at: i64,
}

impl TaskRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      number: row.get(1)?,
      title: row.get(2)?,
      brief: row.get(3)?,
      pipeline: row.get(4)?,
      budget_tokens: row.get(5)?,
      max_duration_ms: row.get(6)?,
      status: row.get(7)?,
      created_at: row.get(8)?,
    })
  }

  fn into_task(self, project: &ProjectKey, acceptance_criteria: Vec<String>) -> Result<Task, StoreError> {
    let limits = self.limits()?;
    let details = TaskDetails {
      title: Name::new(&self.title).map_err(|error| corrupt("task title", error))?,
      brief: self.brief,
      acceptance_criteria,
      pipeline: self
        .pipeline
        .as_deref()
        .map(|id| parse("pipeline id", id))
        .transpose()?,
      limits,
    };
    Ok(Task::from_stored(
      parse("task id", &self.id)?,
      TaskKey::new(project.clone(), task_number(self.number)?),
      details,
      parse_status(&self.status)?,
      Timestamp::from_unix_millis(self.created_at),
    ))
  }

  fn limits(&self) -> Result<Option<TaskLimits>, StoreError> {
    let (Some(tokens), Some(duration_ms)) = (self.budget_tokens, self.max_duration_ms) else {
      return Ok(None);
    };
    TaskLimits::new(
      budget_value("task budget", tokens)?,
      duration_value("task time limit", duration_ms)?,
    )
    .map(Some)
    .map_err(|error| corrupt("task limits", error))
  }
}

fn limit_columns(limits: Option<TaskLimits>) -> Result<(Option<i64>, Option<i64>), StoreError> {
  let Some(limits) = limits else {
    return Ok((None, None));
  };
  Ok((
    Some(tokens_column("task budget", limits.budget().get())?),
    Some(millis_column("task time limit", limits.max_duration())?),
  ))
}

fn task_number(number: i64) -> Result<NonZeroU32, StoreError> {
  u32::try_from(number)
    .ok()
    .and_then(NonZeroU32::new)
    .ok_or_else(|| corrupt("task number", number))
}

const fn status_text(status: TaskStatus) -> &'static str {
  match status {
    TaskStatus::Draft => "draft",
    TaskStatus::Queued => "queued",
  }
}

fn parse_status(value: &str) -> Result<TaskStatus, StoreError> {
  match value {
    "draft" => Ok(TaskStatus::Draft),
    "queued" => Ok(TaskStatus::Queued),
    other => Err(corrupt("task status", other)),
  }
}
