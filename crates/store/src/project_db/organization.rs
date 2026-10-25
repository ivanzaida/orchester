use std::collections::HashMap;

use orchester_core::{
  id::{RunId, TaskId},
  task::organization::{
    ChangeActor, DependencyGate, OrganizationChange, OrganizationError, TaskOrganization, WorkflowStatus,
  },
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{decode_json, encode_json},
};

impl ProjectDb {
  pub fn task_organization_updated_at(&self, task: TaskId) -> Result<Option<Timestamp>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT updated_at FROM task_organization WHERE task_id = ?1",
        [task.to_string()],
        |row| row.get::<_, i64>(0),
      )
      .optional()
      .map(|value| value.map(Timestamp::from_unix_millis))
      .map_err(StoreError::from)
  }
  pub fn task_organization(&self, task: TaskId) -> Result<TaskOrganization, StoreError> {
    load(&self.connection, task)
  }

  pub fn active_task_run(&self, task: TaskId) -> Result<Option<RunId>, StoreError> {
    active_run(&self.connection, task)
  }

  pub fn set_task_organization(
    &mut self,
    task: TaskId,
    value: &TaskOrganization,
    actor: ChangeActor,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    validate(&transaction, task, value)?;
    write(&transaction, task, value, actor, at)?;
    transaction.commit()?;
    Ok(())
  }

  /// Applies `update` to what `task` has stored, in one transaction; whether that changed it. A
  /// bulk change calls it per task, so each task's failure is its own.
  pub fn update_task_organization(
    &mut self,
    task: TaskId,
    actor: ChangeActor,
    at: Timestamp,
    update: impl FnOnce(TaskOrganization) -> TaskOrganization,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let before = load(&transaction, task)?;
    let value = update(before.clone());
    validate(&transaction, task, &value)?;
    write(&transaction, task, &value, actor, at)?;
    transaction.commit()?;
    Ok(value != before)
  }

  /// Sets `task` to the closed `status` (Done or Canceled) as `actor`'s change, in one transaction with the check
  /// that no run of it has not ended: a run that started since the person confirmed is named
  /// ([`OrganizationError::StopRunFirst`]) instead of left running under a closed task. Whether that changed it.
  pub fn close_task_organization(
    &mut self,
    task: TaskId,
    status: WorkflowStatus,
    (actor, at): (ChangeActor, Timestamp),
  ) -> Result<bool, StoreError> {
    let transaction = self
      .connection
      .transaction_with_behavior(TransactionBehavior::Immediate)?;
    if let Some(run) = active_run(&transaction, task)? {
      return Err(OrganizationError::StopRunFirst(run).into());
    }
    let before = load(&transaction, task)?;
    let value = before.clone().with_status(status);
    validate(&transaction, task, &value)?;
    write(&transaction, task, &value, actor, at)?;
    transaction.commit()?;
    Ok(value != before)
  }

  pub fn task_organization_history(&self, task: TaskId) -> Result<Vec<OrganizationChange>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT actor, changed_at, old_value, new_value, old_tag_names, new_tag_names FROM organization_changes WHERE task_id = ?1 ORDER BY sequence",
    )?;
    let rows = statement
      .query_map([task.to_string()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, i64>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, String>(3)?,
          row.get::<_, String>(4)?,
          row.get::<_, String>(5)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(actor, at, from, to, old_names, new_names)| {
        Ok(OrganizationChange {
          actor: decode_json("organization actor", 1, &actor)?,
          at: Timestamp::from_unix_millis(at),
          from: decode_json("organization history", 1, &from)?,
          to: decode_json("organization history", 1, &to)?,
          from_tag_names: decode_json("tag history names", 1, &old_names)?,
          to_tag_names: decode_json("tag history names", 1, &new_names)?,
        })
      })
      .collect()
  }

  /// Only linked unfinished tasks prevent Run. The manual reason remains informational.
  pub fn unfinished_blocking_tasks(&self, task: TaskId) -> Result<Vec<TaskId>, StoreError> {
    unfinished(&self.connection, task)
  }

  pub fn check_task_readiness(&self, task: TaskId) -> Result<(), StoreError> {
    check_readiness(&self.connection, task)
  }
}

pub(super) fn load(connection: &Connection, task: TaskId) -> Result<TaskOrganization, StoreError> {
  ensure_task(connection, task)?;
  let body: Option<String> = connection
    .query_row(
      "SELECT body FROM task_organization WHERE task_id = ?1",
      [task.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  body.map_or_else(
    || Ok(TaskOrganization::default()),
    |body| decode_json("task organization", 1, &body),
  )
}

pub(super) fn write(
  connection: &Connection,
  task: TaskId,
  value: &TaskOrganization,
  actor: ChangeActor,
  at: Timestamp,
) -> Result<(), StoreError> {
  let before = load(connection, task)?;
  if before == *value {
    return Ok(());
  }
  super::sharing::ensure_local_task(connection, task)?;
  let body = encode_json("task organization", 1, value)?;
  connection.execute("INSERT INTO task_organization(task_id, body, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(task_id) DO UPDATE SET body = excluded.body, updated_at = excluded.updated_at", params![task.to_string(), body, at.unix_millis()])?;
  connection.execute(
    "INSERT INTO organization_changes(task_id, actor, changed_at, old_value, new_value, old_tag_names, new_tag_names) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![
      task.to_string(),
      encode_json("organization actor", 1, &actor)?,
      at.unix_millis(),
      encode_json("task organization", 1, &before)?,
      body,
      tag_names(connection, &before)?,
      tag_names(connection, value)?
    ],
  )?;
  connection.execute(
    "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
    [task.to_string()],
  )?;
  Ok(())
}

/// Stores the organization of a shared task as the server confirmed it, changed at `at`. The
/// server keeps its history; nothing is recorded here.
pub(super) fn store_cached(
  connection: &Connection,
  task: TaskId,
  value: &TaskOrganization,
  at: Timestamp,
) -> Result<(), StoreError> {
  let body = encode_json("task organization", 1, value)?;
  connection.execute(
    "INSERT INTO task_organization(task_id, body, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(task_id) DO UPDATE      SET body = excluded.body, updated_at = excluded.updated_at",
    params![task.to_string(), body, at.unix_millis()],
  )?;
  Ok(())
}

pub(super) fn automatic(
  connection: &Connection,
  task: TaskId,
  status: WorkflowStatus,
  at: Timestamp,
) -> Result<(), StoreError> {
  // The server has a shared task's organization: its holder's desktop sends the change.
  if super::shared_tasks::is_shared(connection, task)? {
    let from = load(connection, task)?.status();
    return super::automatic_status::record(connection, task, (from, status), at);
  }
  let value = load(connection, task)?.with_status(status);
  write(connection, task, &value, ChangeActor::Orchester, at)
}

fn ensure_task(connection: &Connection, task: TaskId) -> Result<(), StoreError> {
  let exists: bool = connection.query_row(
    "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1)",
    [task.to_string()],
    |row| row.get(0),
  )?;
  if !exists {
    return Err(StoreError::TaskNotFound(task));
  }
  Ok(())
}

/// The latest run of `task` that has not ended. An interrupted run whose worktree was removed never
/// continues, so it counts as ended (ORC-63).
pub(super) fn active_run(connection: &Connection, task: TaskId) -> Result<Option<RunId>, StoreError> {
  let id: Option<String> = connection
    .query_row(
      "SELECT id FROM runs WHERE task_id = ?1 AND status NOT IN ('succeeded', 'failed', 'cancelled') \
       AND NOT (status = 'interrupted' AND id IN (SELECT run_id FROM run_worktree_removals)) \
       ORDER BY number DESC LIMIT 1",
      [task.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  id.map(|id| crate::rows::parse("run id", &id)).transpose()
}

pub(super) fn validate(connection: &Connection, task: TaskId, value: &TaskOrganization) -> Result<(), StoreError> {
  // Only a change to Canceled stops at an active run; a Canceled task's other fields stay editable.
  if load(connection, task)?.status() != value.status() {
    value.status().check_change(active_run(connection, task)?)?;
  }
  for tag in value.tags() {
    super::tags::ensure_tag(connection, *tag)?;
  }
  let mut graph = HashMap::new();
  let mut pending: Vec<_> = value.dependencies().iter().map(|link| link.task).collect();
  while let Some(id) = pending.pop() {
    if graph.contains_key(&id) {
      continue;
    }
    let other = load(connection, id)?;
    pending.extend(other.dependencies().iter().map(|link| link.task));
    graph.insert(id, other);
  }
  value.check_dependencies(task, &graph)?;
  Ok(())
}

/// The blocking tasks of `task` that have not finished. This is `DependencyGate::is_completed` of
/// each blocking task's phase, read in SQL because every task list and run start asks it; the
/// test `organization_unfinished_follows_the_dependency_gate_rule` keeps the two the same.
pub(super) fn unfinished(connection: &Connection, task: TaskId) -> Result<Vec<TaskId>, StoreError> {
  let value = load(connection, task)?;
  let mut blocking = Vec::new();
  for link in value.dependencies() {
    let complete: bool = match link.gate {
      DependencyGate::Review => connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM runs WHERE id = (SELECT id FROM runs WHERE task_id = ?1 ORDER BY number DESC LIMIT 1) AND status = 'succeeded')",
        [link.task.to_string()],
        |row| row.get(0),
      )?,
      DependencyGate::Merge => connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM run_merges WHERE run_id = (SELECT id FROM runs WHERE task_id = ?1 ORDER BY number DESC LIMIT 1))",
        [link.task.to_string()],
        |row| row.get(0),
      )?,
    };
    if !complete {
      blocking.push(link.task);
    }
  }
  Ok(blocking)
}

fn tag_names(connection: &Connection, value: &TaskOrganization) -> Result<String, StoreError> {
  let mut names = Vec::new();
  for id in value.tags() {
    let name: String =
      connection.query_row("SELECT name FROM project_tags WHERE id = ?1", [id.to_string()], |row| {
        row.get(0)
      })?;
    names.push(name);
  }
  encode_json("tag history names", 1, &names)
}

pub(super) fn check_readiness(connection: &Connection, task: TaskId) -> Result<(), StoreError> {
  let blocking = unfinished(connection, task)?;
  if blocking.is_empty() {
    Ok(())
  } else {
    Err(OrganizationError::Blocked(blocking).into())
  }
}
