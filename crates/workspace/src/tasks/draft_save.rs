//! Saving the task form: a new draft or an existing task in place. Every function blocks on SQLite; the UI
//! calls them off the UI thread.

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  sharing::TaskRevision,
  store::TaskStore,
  task::{Task, TaskDetails, TaskStatus, TaskUpdate},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

/// Saves `details` as one draft task under `batch`, the key the form generated for this
/// save. Retrying with the same key never creates a second task: if an earlier attempt was
/// stored after all, that task is returned, with the details of this attempt if they were
/// edited since and the task is still a draft.
pub fn save_draft<S: TaskStore>(
  store: &mut S,
  batch: IdempotencyKey,
  details: TaskDetails,
  now: Timestamp,
) -> Result<TaskUpdate, S::Error> {
  let task = store
    .create_tasks(batch, vec![details.clone()], now)?
    .into_iter()
    .next()
    .expect("create_tasks returns one task for each of the one details given");
  if task.details() == &details || task.status() != TaskStatus::Draft {
    return Ok(TaskUpdate::Saved(task));
  }
  let edited = Task::from_stored(
    task.id(),
    task.key().clone(),
    details,
    task.status(),
    task.pipeline_version(),
    task.created_at(),
  )
  .with_organization(task.organization().clone())
  .with_module(task.module())
  .with_revision(task.revision());
  store.save_task(&edited)
}

/// Where the task form saves: a new draft under `New`, or an existing task in place, as it was
/// read at the revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveTarget {
  New(IdempotencyKey),
  Task(TaskId, TaskRevision),
}

/// Saves `details` as a new draft or over an existing task in place.
pub fn save_to<S: TaskStore>(
  store: &mut S,
  target: SaveTarget,
  details: TaskDetails,
  now: Timestamp,
) -> Result<TaskUpdate, S::Error> {
  match target {
    SaveTarget::New(batch) => save_draft(store, batch, details, now),
    SaveTarget::Task(task, read) => store.update_task(task, read, details),
  }
}

/// A Save as draft request cannot succeed when its idempotency key now names a queued task.
#[derive(Debug, Error)]
pub enum TaskDraftSaveError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The shared project's server did not take the save.
  #[error(transparent)]
  Write(Box<super::writer::WriteError>),
  #[error("the task is no longer a draft: {0:?}")]
  NotDraft(TaskStatus),
  #[error("the persisted task differs from the requested details")]
  DetailsMismatch,
  /// The task was changed since the form read it; nothing was saved.
  #[error("the task was changed since it was read")]
  Changed(Box<Task>),
}

impl From<super::writer::WriteError> for TaskDraftSaveError {
  fn from(error: super::writer::WriteError) -> Self {
    match error {
      super::writer::WriteError::Store(error) => Self::Store(error),
      error => Self::Write(Box::new(error)),
    }
  }
}

/// [`save_to`] in the project database of `repo_root`, confirmed as the requested details: the
/// saved task.
pub fn save_draft_in(repo_root: &Path, target: SaveTarget, details: &TaskDetails) -> Result<Task, TaskDraftSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let saved = match save_to(&mut db, target, details.clone(), Timestamp::now())? {
    TaskUpdate::Saved(task) => task,
    TaskUpdate::Conflict { current } => return Err(TaskDraftSaveError::Changed(Box::new(current))),
  };
  if matches!(target, SaveTarget::New(_)) && saved.status() != TaskStatus::Draft {
    return Err(TaskDraftSaveError::NotDraft(saved.status()));
  }
  if saved.details() != details {
    return Err(TaskDraftSaveError::DetailsMismatch);
  }
  Ok(saved)
}
