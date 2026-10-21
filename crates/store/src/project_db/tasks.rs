use std::num::NonZeroU32;

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  name::Name,
  project::{ProjectKey, TaskKey},
  resource::VersionNumber,
  sharing::TaskRevision,
  store::TaskStore,
  task::{Task, TaskDetails, TaskLimits, TaskStatus, TaskUpdate},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Params, Row, Transaction, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{
    corrupt, decode_json, encode_json, optional_budget_column, optional_budget_value, optional_duration_value,
    optional_millis_column, parse, tokens_column,
  },
};

/// A shared task names its pipeline by slug (`docs/architecture/server.md`, "Pipelines by name"): it
/// is this computer's pipeline of that slug, or none when it has none. A *Not shared* task keeps its
/// own pipeline and the key it had before the project joined (`kept_key`).
const SELECT_TASKS: &str = "SELECT id, number, title, brief, CASE WHEN sharing IS 'shared' THEN \
  (SELECT resources.id FROM resources WHERE resources.kind = 'pipeline' AND resources.slug = tasks.pipeline_slug) \
  ELSE pipeline_id END, budget_tokens, max_duration_ms, status, created_at, pipeline_version, revision, kept_key, \
  (SELECT module_id FROM task_modules WHERE task_modules.task_id = tasks.id) \
  FROM tasks";

struct RunTaskSnapshot {
  title: String,
  brief: String,
  criteria: String,
  pipeline: Option<String>,
  budget: Option<i64>,
  duration: Option<i64>,
}

impl ProjectDb {
  /// Finds the task already created by an idempotent form submission, if any.
  pub fn task_by_batch(&self, batch: IdempotencyKey) -> Result<Option<Task>, StoreError> {
    let mut found = load_tasks(
      &self.connection,
      self.project.key(),
      "WHERE batch_key = ?1 ORDER BY number LIMIT 1",
      [batch.to_string()],
    )?;
    Ok(found.pop())
  }

  /// The task content a run started with, including after the task is edited for a later run.
  /// A run from before snapshots existed uses current details until its first subsequent edit.
  /// Status and organization (board status, priority, tags) are the task's current ones.
  pub fn task_for_run(&self, task_id: TaskId, run_id: RunId) -> Result<Option<Task>, StoreError> {
    let Some(task) = self.task(task_id)? else {
      return Ok(None);
    };
    let belongs_to_task: bool = self.connection.query_row(
      "SELECT EXISTS(SELECT 1 FROM runs WHERE id = ?1 AND task_id = ?2)",
      params![run_id.to_string(), task_id.to_string()],
      |row| row.get(0),
    )?;
    if !belongs_to_task {
      return Ok(None);
    }
    let snapshot: Option<RunTaskSnapshot> = self.connection.query_row(
      "SELECT title, brief, criteria, pipeline_id, budget_tokens, max_duration_ms FROM run_task_snapshots WHERE run_id = ?1",
      [run_id.to_string()],
      |row| Ok(RunTaskSnapshot {
        title: row.get(0)?,
        brief: row.get(1)?,
        criteria: row.get(2)?,
        pipeline: row.get(3)?,
        budget: row.get(4)?,
        duration: row.get(5)?,
      }),
    ).optional()?;
    let Some(snapshot) = snapshot else {
      return Ok(Some(task));
    };
    let mut details = task.details().clone();
    details.title = Name::new(&snapshot.title).map_err(|error| corrupt("run task title", error))?;
    details.brief = snapshot.brief;
    details.acceptance_criteria = decode_json("run task criteria", 1, &snapshot.criteria)?;
    details.pipeline = snapshot
      .pipeline
      .as_deref()
      .map(|id| parse("run task pipeline", id))
      .transpose()?;
    details.limits = task_limits(snapshot.budget, snapshot.duration)?;
    let organization = task.organization().clone();
    let estimate = task.estimate().cloned();
    Ok(Some(
      Task::from_stored(
        task.id(),
        task.key().clone(),
        details,
        task.status(),
        task.pipeline_version(),
        task.created_at(),
      )
      .with_organization(organization)
      .with_estimate(estimate)
      .with_module(task.module())
      .with_revision(task.revision()),
    ))
  }
}

/// Records the task prompt content for a newly created run, inside its creation transaction.
pub(super) fn snapshot_new_run(
  transaction: &Transaction<'_>,
  run: RunId,
  task_id: TaskId,
  project: &ProjectKey,
) -> Result<(), StoreError> {
  let task = load_tasks(transaction, project, "WHERE id = ?1", [task_id.to_string()])?
    .pop()
    .ok_or(StoreError::TaskNotFound(task_id))?;
  let (budget, duration) = limit_columns(task.details().limits)?;
  transaction.execute(
    "INSERT INTO run_task_snapshots (run_id, title, brief, criteria, pipeline_id, budget_tokens, max_duration_ms) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![
      run.to_string(),
      task.details().title.as_str(),
      task.details().brief,
      encode_json("run task criteria", 1, &task.details().acceptance_criteria)?,
      task.details().pipeline.map(|id| id.to_string()),
      budget,
      duration
    ],
  )?;
  Ok(())
}

/// Existing runs from before this schema acquire their original prompt content before an edit.
pub(super) fn snapshot_earlier_runs(transaction: &Transaction<'_>, task: &Task) -> Result<(), StoreError> {
  let (budget, duration) = limit_columns(task.details().limits)?;
  transaction.execute(
    "INSERT INTO run_task_snapshots (run_id, title, brief, criteria, pipeline_id, budget_tokens, max_duration_ms) \
     SELECT id, ?2, ?3, ?4, ?5, ?6, ?7 FROM runs WHERE task_id = ?1 \
     AND id NOT IN (SELECT run_id FROM run_task_snapshots)",
    params![
      task.id().to_string(),
      task.details().title.as_str(),
      task.details().brief,
      encode_json("run task criteria", 1, &task.details().acceptance_criteria)?,
      task.details().pipeline.map(|id| id.to_string()),
      budget,
      duration
    ],
  )?;
  Ok(())
}

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
    super::sharing::ensure_local_project(&transaction)?;
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

  /// The project's task list: without the tasks kept *Not shared* when it joined a server project.
  fn tasks(&self) -> Result<Vec<Task>, StoreError> {
    load_tasks(
      &self.connection,
      self.project.key(),
      "WHERE sharing IS NOT 'not_shared' ORDER BY number",
      [],
    )
  }

  fn save_task(&mut self, task: &Task) -> Result<TaskUpdate, StoreError> {
    let transaction = self.connection.transaction()?;
    let current = load_task(&transaction, self.project.key(), task.id())?;
    if current.revision() != task.revision() {
      return Ok(TaskUpdate::Conflict { current });
    }
    let revision = update_task(&transaction, task)?;
    transaction.commit()?;
    Ok(TaskUpdate::Saved(task.clone().with_revision(revision)))
  }

  fn update_task(&mut self, id: TaskId, read: TaskRevision, details: TaskDetails) -> Result<TaskUpdate, StoreError> {
    let transaction = self.connection.transaction()?;
    let current = load_task(&transaction, self.project.key(), id)?;
    if current.revision() != read {
      return Ok(TaskUpdate::Conflict { current });
    }
    let task = edit_task(&transaction, current, details)?;
    transaction.commit()?;
    Ok(TaskUpdate::Saved(task))
  }
}

/// The stored task `id`, read in `connection`.
pub(super) fn load_task(connection: &Connection, project: &ProjectKey, id: TaskId) -> Result<Task, StoreError> {
  load_tasks(connection, project, "WHERE id = ?1", [id.to_string()])?
    .pop()
    .ok_or(StoreError::TaskNotFound(id))
}

/// Replaces `task`'s details within `transaction`: a task with runs stays complete, and its
/// earlier runs keep the details they ran with.
pub(super) fn edit_task(
  transaction: &Transaction<'_>,
  mut task: Task,
  details: TaskDetails,
) -> Result<Task, StoreError> {
  let has_run: bool = transaction.query_row(
    "SELECT EXISTS (SELECT 1 FROM runs WHERE task_id = ?1)",
    [task.id().to_string()],
    |row| row.get(0),
  )?;
  let before = task.clone();
  task
    .edit(details, has_run)
    .map_err(|source| StoreError::TaskNotEditable {
      task: task.id(),
      source,
    })?;
  if has_run {
    snapshot_earlier_runs(transaction, &before)?;
  }
  let revision = update_task(transaction, &task)?;
  Ok(task.with_revision(revision))
}

/// Stores the details and status of `task`, which must exist and be this project's own, and
/// raises its revision; returns the new revision. A shared task changes only on the server.
pub(super) fn update_task(transaction: &Transaction<'_>, task: &Task) -> Result<TaskRevision, StoreError> {
  let details = task.details();
  let (budget_tokens, max_duration_ms) = limit_columns(details.limits)?;
  let revision: Option<i64> = transaction
    .query_row(
      "UPDATE tasks SET title = ?2, brief = ?3, pipeline_id = ?4, budget_tokens = ?5, max_duration_ms = ?6, \
       status = ?7, pipeline_version = ?8, revision = revision + 1 WHERE id = ?1 AND sharing IS NULL \
       RETURNING revision",
      params![
        task.id().to_string(),
        details.title.as_str(),
        details.brief,
        details.pipeline.map(|id| id.to_string()),
        budget_tokens,
        max_duration_ms,
        status_text(task.status()),
        task.pipeline_version().map(|version| version.get()),
      ],
      |row| row.get(0),
    )
    .optional()?;
  let Some(revision) = revision else {
    super::sharing::ensure_local_task(transaction, task.id())?;
    return Err(StoreError::TaskNotFound(task.id()));
  };
  write_criteria(transaction, task.id(), &details.acceptance_criteria)?;
  revision_value(revision)
}

pub(super) fn revision_value(revision: i64) -> Result<TaskRevision, StoreError> {
  u64::try_from(revision)
    .ok()
    .and_then(|revision| TaskRevision::try_from(revision).ok())
    .ok_or_else(|| corrupt("task revision", revision))
}

/// Stores the new task `task`, and keeps what it asks for as its creation version.
pub(super) fn insert_task(transaction: &Transaction<'_>, task: &Task, batch: &str) -> Result<(), StoreError> {
  let details = task.details();
  let (budget_tokens, max_duration_ms) = limit_columns(details.limits)?;
  transaction.execute(
    "INSERT INTO tasks (id, number, batch_key, title, brief, pipeline_id, budget_tokens, max_duration_ms, status, \
     created_at, pipeline_version, revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
      task.pipeline_version().map(|version| version.get()),
      tokens_column("task revision", task.revision().get())?,
    ],
  )?;
  write_criteria(transaction, task.id(), &details.acceptance_criteria)?;
  super::task_snapshots::record_creation(transaction, task)
}

pub(super) fn write_criteria(
  transaction: &Transaction<'_>,
  task: TaskId,
  criteria: &[String],
) -> Result<(), StoreError> {
  let task = task.to_string();
  transaction.execute("DELETE FROM task_criteria WHERE task_id = ?1", [&task])?;
  let mut insert = transaction.prepare("INSERT INTO task_criteria (task_id, position, text) VALUES (?1, ?2, ?3)")?;
  for (position, criterion) in criteria.iter().enumerate() {
    let position = i64::try_from(position).map_err(|_| StoreError::Unrepresentable("criterion position"))?;
    insert.execute(params![task, position, criterion])?;
  }
  Ok(())
}

pub(super) fn load_tasks(
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
      let task = row.into_task(project, criteria)?;
      let value = super::organization::load(connection, task.id())?;
      let estimate = super::estimates::load(connection, task.id())?;
      Ok(task.with_organization(value).with_estimate(estimate))
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
  pipeline_version: Option<i64>,
  revision: i64,
  kept_key: Option<String>,
  module: Option<String>,
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
      pipeline_version: row.get(9)?,
      revision: row.get(10)?,
      kept_key: row.get(11)?,
      module: row.get(12)?,
    })
  }

  fn into_task(self, project: &ProjectKey, acceptance_criteria: Vec<String>) -> Result<Task, StoreError> {
    let limits = task_limits(self.budget_tokens, self.max_duration_ms)?;
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
    let pipeline_version = self.pipeline_version.map(version_number).transpose()?;
    let project = match self.kept_key.as_deref() {
      Some(kept) => ProjectKey::new(kept).map_err(|error| corrupt("kept task key", error))?,
      None => project.clone(),
    };
    let module = self.module.as_deref().map(|id| parse("module id", id)).transpose()?;
    Ok(
      Task::from_stored(
        parse("task id", &self.id)?,
        TaskKey::new(project, task_number(self.number)?),
        details,
        parse_status(&self.status)?,
        pipeline_version,
        Timestamp::from_unix_millis(self.created_at),
      )
      .with_module(module)
      .with_revision(revision_value(self.revision)?),
    )
  }
}

/// A task's limits from its budget and time limit columns, each `NULL` for no limit.
pub(super) fn task_limits(budget: Option<i64>, duration: Option<i64>) -> Result<TaskLimits, StoreError> {
  TaskLimits::new(
    optional_budget_value("task budget", budget)?,
    optional_duration_value("task time limit", duration)?,
  )
  .map_err(|error| corrupt("task limits", error))
}

pub(super) fn limit_columns(limits: TaskLimits) -> Result<(Option<i64>, Option<i64>), StoreError> {
  Ok((
    optional_budget_column("task budget", limits.budget())?,
    optional_millis_column("task time limit", limits.max_duration())?,
  ))
}

pub(super) fn task_number(number: i64) -> Result<NonZeroU32, StoreError> {
  u32::try_from(number)
    .ok()
    .and_then(NonZeroU32::new)
    .ok_or_else(|| corrupt("task number", number))
}

fn version_number(number: i64) -> Result<VersionNumber, StoreError> {
  u32::try_from(number)
    .ok()
    .and_then(NonZeroU32::new)
    .map(VersionNumber::new)
    .ok_or_else(|| corrupt("task pipeline version", number))
}

/// Whether `task` is queued, read in `transaction`: a run is recorded only for a queued task.
pub(super) fn is_queued(transaction: &rusqlite::Transaction<'_>, task: TaskId) -> Result<bool, StoreError> {
  Ok(transaction.query_row(
    "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1 AND status = ?2)",
    [task.to_string(), status_text(TaskStatus::Queued).to_owned()],
    |row| row.get(0),
  )?)
}

/// Whether `task` is at `revision`, read in `transaction`: a run confirmed on that revision is recorded only
/// while no edit came since.
pub(super) fn has_revision(
  transaction: &rusqlite::Transaction<'_>,
  task: TaskId,
  revision: TaskRevision,
) -> Result<bool, StoreError> {
  let revision = i64::try_from(revision.get()).map_err(|_| corrupt("task revision", revision.get()))?;
  Ok(transaction.query_row(
    "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1 AND revision = ?2)",
    params![task.to_string(), revision],
    |row| row.get(0),
  )?)
}

/// The `tasks.status` value: draft or submitted ([`TaskStatus`]), which no run event changes.
pub(super) const fn status_text(status: TaskStatus) -> &'static str {
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
