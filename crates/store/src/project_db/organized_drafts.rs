use orchester_core::{
  id::{IdempotencyKey, TaskId},
  project::TaskKey,
  task::{
    Task, TaskDetails, TaskStatus,
    organization::{ChangeActor, TaskOrganization},
  },
  time::Timestamp,
};

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
      tasks::edit_task(&transaction, task, details)?
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

  /// Saves an edited task: its details, and the organization changes the person made from `base`
  /// (what the form loaded) to `edited`, over what is stored now ([`TaskOrganization::rebase`]),
  /// so a status Orchester or a bulk change set meanwhile is not overwritten.
  pub fn update_organized_task(
    &mut self,
    id: TaskId,
    details: TaskDetails,
    (base, edited): (&TaskOrganization, &TaskOrganization),
    at: Timestamp,
  ) -> Result<Task, StoreError> {
    let transaction = self.connection.transaction()?;
    let task = tasks::load_tasks(&transaction, self.project.key(), "WHERE id = ?1", [id.to_string()])?
      .pop()
      .ok_or(StoreError::TaskNotFound(id))?;
    let value = edited.rebase(base, &organization::load(&transaction, id)?);
    organization::validate(&transaction, id, &value)?;
    let task = tasks::edit_task(&transaction, task, details)?;
    organization::write(&transaction, id, &value, ChangeActor::Person, at)?;
    transaction.commit()?;
    Ok(task.with_organization(value))
  }
}
