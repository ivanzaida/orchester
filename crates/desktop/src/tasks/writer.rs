//! Where the desktop's changes of tasks go (`docs/architecture/server.md`, "The desktop's trait
//! boundary"): a local project's into its database, a shared project's to the server through
//! [`RemoteTaskStore`], whose cache the database is. Both are a [`TaskStore`]; the organization and
//! the project's tags change through the same writer, so no screen changes a shared task only on
//! this computer. Every method blocks; the UI calls them off the UI thread.

use std::path::Path;

use orchester_api::client::ServerAddress;
use orchester_core::{
  id::{IdempotencyKey, TagId, TaskId},
  name::Name,
  sharing::TaskRevision,
  store::TaskStore,
  task::{
    Task, TaskDetails, TaskUpdate,
    organization::{ChangeActor, Tag, TaskOrganization},
  },
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::data::SaveTarget;
use crate::{
  messages::{self, Message},
  sharing::{
    Servers, SharingError,
    project::{ProjectServer, store::RemoteTaskStore},
  },
};

/// Why a change of a task was not made.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The server of the shared project did not take it.
  #[error("{}", .0.source)]
  Sharing(Box<SharingFailure>),
}

/// What the server of a shared project refused, or why it could not be asked; `server` is its
/// address when it is in the list of servers.
#[derive(Debug)]
pub struct SharingFailure {
  pub source: SharingError,
  pub server: Option<ServerAddress>,
}

impl WriteError {
  pub fn sharing(source: SharingError, server: Option<ServerAddress>) -> Self {
    match source {
      SharingError::Store(error) => Self::Store(error),
      source => Self::Sharing(Box::new(SharingFailure { source, server })),
    }
  }

  pub fn message(&self) -> Message {
    let Self::Sharing(failure) = self else {
      return self.store().map_or_else(messages::interrupted, messages::store_error);
    };
    match (&failure.source, &failure.server) {
      (source, Some(server)) => source.message(server),
      (SharingError::ServerMissing, None) => Message::new("sharing.errors.serverMissing"),
      (SharingError::KeptChanging, None) => Message::new("sharing.errors.keptChanging"),
      (_, None) => Message::new("sharing.errors.unavailable"),
    }
  }

  /// The local store's error, e.g. one that names a run to stop.
  pub const fn store(&self) -> Option<&StoreError> {
    match self {
      Self::Store(error) => Some(error),
      Self::Sharing(_) => None,
    }
  }
}

/// A project's tasks, for changing them.
pub enum TaskWriter {
  Local(ProjectDb),
  Shared(Box<RemoteTaskStore>),
}

impl TaskWriter {
  /// The writer of the project at `repo_root`: its server when it is shared. Must run on a worker
  /// of the application's runtime.
  pub fn open(repo_root: &Path, servers: &Servers) -> Result<Self, WriteError> {
    let db = ProjectDb::open(repo_root)?;
    if db.sharing()?.is_none() {
      return Ok(Self::Local(db));
    }
    let server = ProjectServer::open(servers, repo_root)
      .map_err(|source| WriteError::sharing(source, None))?
      .ok_or(StoreError::NotShared)?;
    let address = server.entry().address.clone();
    RemoteTaskStore::new(db, server)
      .map(|store| Self::Shared(Box::new(store)))
      .map_err(|source| WriteError::sharing(source, Some(address)))
  }

  /// The project database, for reads: a shared project's cache.
  pub fn db(&self) -> &ProjectDb {
    match self {
      Self::Local(db) => db,
      Self::Shared(store) => store.db(),
    }
  }

  /// Saves `details` and the organization changes the person made from `base` to `edited`, as a
  /// new draft or over a task read at its revision.
  pub fn save_organized(
    &mut self,
    target: SaveTarget,
    details: TaskDetails,
    (base, edited): (&TaskOrganization, &TaskOrganization),
    now: Timestamp,
  ) -> Result<TaskUpdate, WriteError> {
    match (self, target) {
      (Self::Local(db), SaveTarget::New(batch)) => Ok(TaskUpdate::Saved(
        db.create_organized_task(batch, details, edited, now)?,
      )),
      (Self::Local(db), SaveTarget::Task(task, read)) => {
        Ok(db.update_organized_task(task, read, details, (base, edited), now)?)
      }
      (Self::Shared(store), SaveTarget::New(batch)) => {
        let created = store.create_organized(batch, &details, edited);
        Ok(TaskUpdate::Saved(shared(store, created)?))
      }
      (Self::Shared(store), SaveTarget::Task(task, read)) => {
        check_cancel(store.db(), task, base, edited)?;
        let update = store.update_organized(task, read, &details, (base, edited));
        shared(store, update)
      }
    }
  }

  /// Applies `update` to the organization of `task` as stored now; whether it changed it.
  pub fn update_organization(
    &mut self,
    task: TaskId,
    update: impl FnOnce(TaskOrganization) -> TaskOrganization,
  ) -> Result<bool, WriteError> {
    match self {
      Self::Local(db) => Ok(db.update_task_organization(task, ChangeActor::Person, Timestamp::now(), update)?),
      Self::Shared(store) => {
        let base = store.db().task_organization(task)?;
        let edited = update(base.clone());
        check_cancel(store.db(), task, &base, &edited)?;
        let changed = store.change_organization(task, (&base, &edited), ChangeActor::Person);
        shared(store, changed)
      }
    }
  }

  pub fn create_tag(&mut self, name: Name) -> Result<Tag, WriteError> {
    match self {
      Self::Local(db) => Ok(db.create_project_tag(name)?),
      Self::Shared(store) => {
        let created = store.create_tag(&name);
        shared(store, created)
      }
    }
  }

  pub fn rename_tag(&mut self, id: TagId, name: &Name) -> Result<(), WriteError> {
    match self {
      Self::Local(db) => Ok(db.rename_project_tag(id, name)?),
      Self::Shared(store) => {
        let renamed = store.rename_tag(id, name);
        shared(store, renamed)
      }
    }
  }

  pub fn merge_tags(&mut self, source: TagId, target: TagId) -> Result<(), WriteError> {
    match self {
      Self::Local(db) => Ok(db.merge_project_tags(source, target, Timestamp::now())?),
      Self::Shared(store) => {
        let merged = store.merge_tags(source, target);
        shared(store, merged)
      }
    }
  }

  pub fn delete_tag(&mut self, id: TagId) -> Result<(), WriteError> {
    match self {
      Self::Local(db) => Ok(db.delete_project_tag(id, Timestamp::now())?),
      Self::Shared(store) => {
        let deleted = store.delete_tag(id);
        shared(store, deleted)
      }
    }
  }
}

impl TaskStore for TaskWriter {
  type Error = WriteError;

  fn create_tasks(
    &mut self,
    batch: IdempotencyKey,
    details: Vec<TaskDetails>,
    now: Timestamp,
  ) -> Result<Vec<Task>, WriteError> {
    match self {
      Self::Local(db) => Ok(db.create_tasks(batch, details, now)?),
      Self::Shared(store) => {
        let created = store.create_tasks(batch, details, now);
        shared(store, created)
      }
    }
  }

  fn task(&self, id: TaskId) -> Result<Option<Task>, WriteError> {
    Ok(self.db().task(id)?)
  }

  fn tasks(&self) -> Result<Vec<Task>, WriteError> {
    Ok(self.db().tasks()?)
  }

  fn save_task(&mut self, task: &Task) -> Result<TaskUpdate, WriteError> {
    match self {
      Self::Local(db) => Ok(db.save_task(task)?),
      Self::Shared(store) => {
        let saved = store.save_task(task);
        shared(store, saved)
      }
    }
  }

  fn update_task(&mut self, id: TaskId, read: TaskRevision, details: TaskDetails) -> Result<TaskUpdate, WriteError> {
    match self {
      Self::Local(db) => Ok(db.update_task(id, read, details)?),
      Self::Shared(store) => {
        let updated = store.update_task(id, read, details);
        shared(store, updated)
      }
    }
  }
}

/// `result` of `store`, its error naming the store's server.
fn shared<T>(store: &RemoteTaskStore, result: Result<T, SharingError>) -> Result<T, WriteError> {
  result.map_err(|source| WriteError::sharing(source, Some(store.server().entry().address.clone())))
}

/// A shared task goes to Canceled only once this computer's run of it stopped: the holder's
/// desktop checks its local run before sending (`docs/architecture/server.md`, "API").
fn check_cancel(
  db: &ProjectDb,
  task: TaskId,
  base: &TaskOrganization,
  edited: &TaskOrganization,
) -> Result<(), WriteError> {
  if base.status() != edited.status() {
    edited
      .status()
      .check_change(db.active_task_run(task)?)
      .map_err(StoreError::from)?;
  }
  Ok(())
}
