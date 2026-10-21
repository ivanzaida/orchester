//! `RemoteTaskStore`, a shared project's [`TaskStore`] (`docs/architecture/server.md`, "The desktop's
//! trait boundary"): it reads the cache in the project database, works offline, and sends every
//! change to the server with the revision it was made on and only the fields that differ from the
//! cached task; what the server confirmed is written to the cache. A change that meets a newer
//! revision is a conflict with the current task, as for the local store. `TaskStore` is synchronous:
//! the store is used on a blocking worker and waits there for each request.

use orchester_api::{ErrorCode, SharedTask, SharingErrorDetails, TaskChanges, TaskPatch, TasksCreate};
use orchester_core::{
  id::{IdempotencyKey, TaskId},
  sharing::TaskRevision,
  store::TaskStore,
  task::{
    Task, TaskDetails, TaskStatus, TaskUpdate,
    organization::{ChangeActor, TaskOrganization},
  },
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use tokio::runtime::Handle;

use super::{ProjectServer, convert};
use crate::sharing::SharingError;

pub struct RemoteTaskStore {
  pub(super) db: ProjectDb,
  pub(super) server: ProjectServer,
  runtime: Handle,
}

impl RemoteTaskStore {
  /// The store of the shared project `server` is the server project of, over its database `db`.
  /// Must be made on a worker of the application's runtime, whose requests it waits for.
  pub fn new(db: ProjectDb, server: ProjectServer) -> Result<Self, SharingError> {
    let runtime = Handle::try_current().map_err(|_| SharingError::Interrupted)?;
    Ok(Self { db, server, runtime })
  }

  /// The cache, for reads.
  pub const fn db(&self) -> &ProjectDb {
    &self.db
  }

  pub const fn server(&self) -> &ProjectServer {
    &self.server
  }

  pub(super) fn block_on<T>(&self, work: impl Future<Output = T>) -> T {
    self.runtime.block_on(work)
  }

  /// Writes `tasks`, as the server confirmed them, to the cache; `batch` is the key they were
  /// created under.
  pub(super) fn cache(&mut self, tasks: &[SharedTask], batch: Option<IdempotencyKey>) -> Result<(), SharingError> {
    let cached = tasks.iter().map(convert::cached).collect::<Result<Vec<_>, _>>()?;
    let batch = batch.map(|batch| batch.to_string());
    self.db.cache_shared_tasks(&cached, batch.as_deref())?;
    Ok(())
  }

  pub(super) fn cached(&self, task: TaskId) -> Result<Task, SharingError> {
    Ok(self.db.task(task)?.ok_or(StoreError::TaskNotFound(task))?)
  }

  /// Sends `changes` of `task`, made on revision `read`: the task the server confirmed, or a
  /// conflict with the current one. Nothing is sent when nothing changed.
  pub(super) fn patch(
    &mut self,
    task: TaskId,
    read: TaskRevision,
    changes: TaskChanges,
  ) -> Result<TaskUpdate, SharingError> {
    self.server.writable()?;
    if convert::is_empty(&changes) {
      let cached = self.cached(task)?;
      if cached.revision() != read {
        return Ok(TaskUpdate::Conflict { current: cached });
      }
      return Ok(TaskUpdate::Saved(cached));
    }
    let patch = TaskPatch {
      revision: read,
      changes,
    };
    let answer = self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.update_task(&token, project, task, &patch).await }),
    );
    match answer {
      Ok(confirmed) => {
        self.cache(std::slice::from_ref(&confirmed), None)?;
        Ok(TaskUpdate::Saved(self.cached(task)?))
      }
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::RevisionConflict) => {
        if let Some(SharingErrorDetails::Revision { current }) = error.details() {
          self.cache(std::slice::from_ref(current), None)?;
        }
        Ok(TaskUpdate::Conflict {
          current: self.cached(task)?,
        })
      }
      Err(error) => Err(error),
    }
  }

  /// Creates tasks of `contents` on the server under `batch`; the server numbers them.
  pub(super) fn create(
    &mut self,
    batch: IdempotencyKey,
    tasks: Vec<orchester_api::TaskContent>,
  ) -> Result<Vec<Task>, SharingError> {
    self.server.writable()?;
    let create = TasksCreate {
      idempotency_key: batch,
      tasks,
    };
    let created = self.block_on(
      self
        .server
        .call(|client, token, project| async move { client.create_tasks(&token, project, &create).await }),
    )?;
    self.cache(&created.tasks, Some(batch))?;
    created.tasks.iter().map(|task| self.cached(task.id)).collect()
  }

  /// Saves `details` of `task`, read at `read`, with the organization changes `actor` made from
  /// `base` to `edited` on top of the organization the cache has now.
  pub fn update_organized(
    &mut self,
    task: TaskId,
    read: TaskRevision,
    details: &TaskDetails,
    (base, edited): (&TaskOrganization, &TaskOrganization),
    actor: ChangeActor,
  ) -> Result<TaskUpdate, SharingError> {
    let cached = self.cached(task)?;
    let changes = convert::content_changes(&self.db, &cached, details, cached.status())?;
    let value = edited.rebase(base, cached.organization());
    let mut changes = convert::with_organization(changes, cached.organization(), &value);
    changes.actor = Some(actor);
    self.patch(task, read, changes)
  }

  /// Creates a draft of `details` organized as `organization` by `actor` under `batch`. The server
  /// records a new task's organization as a person's, so automation's is sent as a change of the
  /// new draft.
  pub fn create_organized(
    &mut self,
    batch: IdempotencyKey,
    details: &TaskDetails,
    organization: &TaskOrganization,
    actor: ChangeActor,
  ) -> Result<Task, SharingError> {
    let first = match actor {
      ChangeActor::Person => organization.clone(),
      ChangeActor::Orchester => TaskOrganization::default(),
    };
    let content = convert::content(&self.db, details, &first)?;
    let task = self
      .create(batch, vec![content])?
      .pop()
      .ok_or(SharingError::InvalidAnswer("created tasks"))?;
    if first == *organization {
      return Ok(task);
    }
    self.change_organization(task.id(), (&first, organization), actor)?;
    self.cached(task.id())
  }
}

impl TaskStore for RemoteTaskStore {
  type Error = SharingError;

  fn create_tasks(
    &mut self,
    batch: IdempotencyKey,
    details: Vec<TaskDetails>,
    _now: Timestamp,
  ) -> Result<Vec<Task>, SharingError> {
    if details.is_empty() {
      return Err(StoreError::EmptyBatch.into());
    }
    let contents = details
      .iter()
      .map(|details| convert::content(&self.db, details, &TaskOrganization::default()))
      .collect::<Result<Vec<_>, _>>()?;
    self.create(batch, contents)
  }

  fn task(&self, id: TaskId) -> Result<Option<Task>, SharingError> {
    Ok(self.db.task(id)?)
  }

  fn tasks(&self) -> Result<Vec<Task>, SharingError> {
    Ok(self.db.tasks()?)
  }

  /// Sends the details and status of `task`; the version of the pipeline a queued task runs stays
  /// on this computer: a version number means nothing on a colleague's.
  fn save_task(&mut self, task: &Task) -> Result<TaskUpdate, SharingError> {
    let cached = self.cached(task.id())?;
    let changes = convert::content_changes(&self.db, &cached, task.details(), task.status())?;
    let update = self.patch(task.id(), task.revision(), changes)?;
    let TaskUpdate::Saved(saved) = update else {
      return Ok(update);
    };
    let pin = (task.status() == TaskStatus::Queued)
      .then(|| task.pipeline_version())
      .flatten();
    if pin != saved.pipeline_version() {
      self.db.pin_shared_task(task.id(), pin)?;
    }
    Ok(TaskUpdate::Saved(self.cached(task.id())?))
  }

  fn update_task(&mut self, id: TaskId, read: TaskRevision, details: TaskDetails) -> Result<TaskUpdate, SharingError> {
    let cached = self.cached(id)?;
    let changes = convert::content_changes(&self.db, &cached, &details, cached.status())?;
    self.patch(id, read, changes)
  }
}
