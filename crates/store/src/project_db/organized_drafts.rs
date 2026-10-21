use orchester_core::{
  id::{IdempotencyKey, TaskId},
  project::TaskKey,
  sharing::TaskRevision,
  task::{
    Task, TaskDetails, TaskStatus, TaskUpdate,
    organization::{ChangeActor, TaskOrganization},
  },
  time::Timestamp,
};

use super::{ProjectDb, organization, tasks};
use crate::StoreError;

impl ProjectDb {
  /// Details, organization and history commit together, including an idempotent new draft; the
  /// history names `actor` as who organized it.
  pub fn create_organized_task(
    &mut self,
    batch: IdempotencyKey,
    details: TaskDetails,
    value: &TaskOrganization,
    (actor, at): (ChangeActor, Timestamp),
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
      super::sharing::ensure_local_project(&transaction)?;
      let number: i64 = transaction.query_row("SELECT next FROM counters WHERE name = 'task'", [], |row| row.get(0))?;
      let key = TaskKey::new(self.project.key().clone(), tasks::task_number(number)?);
      let task = Task::draft(TaskId::generate(), key, details, at);
      tasks::insert_task(&transaction, &task, &batch.to_string())?;
      transaction.execute("UPDATE counters SET next = ?1 WHERE name = 'task'", [number + 1])?;
      task
    };
    organization::validate(&transaction, task.id(), value)?;
    organization::write(&transaction, task.id(), value, actor, at)?;
    let task = tasks::load_task(&transaction, self.project.key(), task.id())?;
    transaction.commit()?;
    Ok(task)
  }

  /// Saves an edited task, read at revision `read`: its details, and the organization changes
  /// `actor` made from `base` (what the form loaded) to `edited`, over what is stored now
  /// ([`TaskOrganization::rebase`]), so a status Orchester or a bulk change set meanwhile is not
  /// overwritten. A task changed since it was read is a conflict, and nothing is stored.
  pub fn update_organized_task(
    &mut self,
    id: TaskId,
    read: TaskRevision,
    details: TaskDetails,
    (base, edited): (&TaskOrganization, &TaskOrganization),
    (actor, at): (ChangeActor, Timestamp),
  ) -> Result<TaskUpdate, StoreError> {
    let transaction = self.connection.transaction()?;
    let task = tasks::load_task(&transaction, self.project.key(), id)?;
    if task.revision() != read {
      return Ok(TaskUpdate::Conflict { current: task });
    }
    let value = edited.rebase(base, &organization::load(&transaction, id)?);
    organization::validate(&transaction, id, &value)?;
    tasks::edit_task(&transaction, task, details)?;
    organization::write(&transaction, id, &value, actor, at)?;
    let task = tasks::load_task(&transaction, self.project.key(), id)?;
    transaction.commit()?;
    Ok(TaskUpdate::Saved(task))
  }
}
