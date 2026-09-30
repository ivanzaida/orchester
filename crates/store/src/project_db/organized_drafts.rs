use orchester_core::{
  id::{IdempotencyKey, TaskId},
  project::TaskKey,
  task::{
    Task, TaskDetails, TaskStatus,
    organization::{ChangeActor, TaskOrganization},
  },
  time::Timestamp,
};
use rusqlite::Transaction;

use super::{ProjectDb, organization, tasks};
use crate::StoreError;

impl ProjectDb {
  /// Details, organization and history commit together, including an idempotent new draft.
  pub fn create_organized_task(
    &mut self,
    batch: IdempotencyKey,
    details: TaskDetails,
    value: &TaskOrganization,
    at: Timestamp,
  ) -> Result<Task, StoreError> {
    let transaction = self.connection.transaction()?;
    let existing = tasks::load_tasks(
      &transaction,
      self.project.key(),
      "WHERE batch_key = ?1 ORDER BY number LIMIT 1",
      [batch.to_string()],
    )?
    .pop();
    let task = if let Some(task) = existing {
      if task.status() != TaskStatus::Draft {
        return Ok(task);
      }
      edit(&transaction, task, details)?
    } else {
      let number: i64 = transaction.query_row("SELECT next FROM counters WHERE name = 'task'", [], |row| row.get(0))?;
      let key = TaskKey::new(self.project.key().clone(), tasks::task_number(number)?);
      let task = Task::draft(TaskId::generate(), key, details, at);
      tasks::insert_task(&transaction, &task, &batch.to_string())?;
      transaction.execute("UPDATE counters SET next = ?1 WHERE name = 'task'", [number + 1])?;
      task
    };
    organization::validate(&transaction, task.id(), value)?;
    organization::write(&transaction, task.id(), value, ChangeActor::Person, at)?;
    transaction.commit()?;
    Ok(task.with_organization(value.clone()))
  }

  pub fn update_organized_task(
    &mut self,
    id: TaskId,
    details: TaskDetails,
    value: &TaskOrganization,
    at: Timestamp,
  ) -> Result<Task, StoreError> {
    let transaction = self.connection.transaction()?;
    let task = tasks::load_tasks(&transaction, self.project.key(), "WHERE id = ?1", [id.to_string()])?
      .pop()
      .ok_or(StoreError::TaskNotFound(id))?;
    organization::validate(&transaction, id, value)?;
    let task = edit(&transaction, task, details)?;
    organization::write(&transaction, id, value, ChangeActor::Person, at)?;
    transaction.commit()?;
    Ok(task.with_organization(value.clone()))
  }
}

fn edit(transaction: &Transaction<'_>, mut task: Task, details: TaskDetails) -> Result<Task, StoreError> {
  let has_run: bool = transaction.query_row(
    "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1)",
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
    tasks::snapshot_earlier_runs(transaction, &before)?;
  }
  tasks::update_task(transaction, &task)?;
  Ok(task)
}
