//! Persistence boundaries. The local SQLite store implements them today; a server client
//! will implement the same traits later, so domain code depends only on these.

use crate::{
  connection::Connection,
  id::{ConnectionId, IdempotencyKey, ResourceId, RunId, TaskId},
  name::Name,
  resource::{Draft, Resource, ResourceBody, ResourceKind, Slug, Version, VersionNumber},
  run::{Checkpoint, NewRun, Run, RunEvent},
  task::{Task, TaskDetails},
  time::Timestamp,
};

pub trait TaskStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Creates `details.len()` draft tasks with consecutive numbers, atomically. Repeating the
  /// call with the same `batch` returns the tasks created the first time instead of new ones.
  fn create_tasks(
    &mut self,
    batch: IdempotencyKey,
    details: Vec<TaskDetails>,
    now: Timestamp,
  ) -> Result<Vec<Task>, Self::Error>;

  fn task(&self, id: TaskId) -> Result<Option<Task>, Self::Error>;

  /// All tasks of the project, ordered by number.
  fn tasks(&self) -> Result<Vec<Task>, Self::Error>;

  /// Persists the details and status of an existing task.
  fn save_task(&mut self, task: &Task) -> Result<(), Self::Error>;
}

/// Versioned configuration resources. Reading a body of the wrong kind is an error, not `None`.
pub trait ResourceStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Creates a resource of kind `B::KIND` whose first draft is `body`, atomically. The slug is
  /// unique among resources of the same kind.
  fn create_resource<B: ResourceBody>(
    &mut self,
    slug: Slug,
    name: Name,
    body: &B,
    now: Timestamp,
  ) -> Result<Resource, Self::Error>;

  fn resource(&self, id: ResourceId) -> Result<Option<Resource>, Self::Error>;

  /// Resources of one kind, ordered by slug.
  fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, Self::Error>;

  /// Stores `draft` as the resource's only draft, replacing an existing one.
  fn save_draft<B: ResourceBody>(&mut self, resource: ResourceId, draft: &Draft<B>) -> Result<(), Self::Error>;

  fn draft<B: ResourceBody>(&self, resource: ResourceId) -> Result<Option<Draft<B>>, Self::Error>;

  /// Publishes the stored draft as the next version and removes the draft, atomically. A draft
  /// not based on the latest version is rejected, see [`crate::resource::publish`].
  fn publish_draft<B: ResourceBody>(&mut self, resource: ResourceId, now: Timestamp)
  -> Result<Version<B>, Self::Error>;

  fn latest_version<B: ResourceBody>(&self, resource: ResourceId) -> Result<Option<Version<B>>, Self::Error>;

  fn version<B: ResourceBody>(
    &self,
    resource: ResourceId,
    number: VersionNumber,
  ) -> Result<Option<Version<B>>, Self::Error>;
}

/// Project connections. Written by the desktop; the runner only reads them.
pub trait ConnectionStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores a new connection. Names are unique within the project.
  fn create_connection(&mut self, connection: &Connection) -> Result<(), Self::Error>;

  /// Stores the current name, executable and last check of an existing connection.
  fn save_connection(&mut self, connection: &Connection) -> Result<(), Self::Error>;

  fn connection(&self, id: ConnectionId) -> Result<Option<Connection>, Self::Error>;

  /// All connections of the project, ordered by name.
  fn connections(&self) -> Result<Vec<Connection>, Self::Error>;
}

/// Execution data, written by the runner. A run changes only by appending the events a
/// transition returned; the resulting state is stored alongside in the same transaction.
pub trait RunStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Creates a pending run of `new.task` with the project's next run number and stores its
  /// creation event, atomically. Repeating the call with the same `key` returns the run
  /// created the first time instead of a new one.
  fn create_run(&mut self, key: IdempotencyKey, new: NewRun, now: Timestamp) -> Result<Run, Self::Error>;

  /// Appends `events`, which must not be empty, and stores `run`, the state after them,
  /// atomically. The first event must directly follow the stored log; otherwise another writer
  /// appended first and the call fails with a sequence conflict, changing nothing.
  fn append_events(&mut self, run: &Run, events: &[RunEvent]) -> Result<(), Self::Error>;

  fn run(&self, id: RunId) -> Result<Option<Run>, Self::Error>;

  /// Runs of a task, ordered by number.
  fn runs_of_task(&self, task: TaskId) -> Result<Vec<Run>, Self::Error>;

  /// The complete event log of a run, in sequence order.
  fn events(&self, run: RunId) -> Result<Vec<RunEvent>, Self::Error>;

  fn latest_checkpoint(&self, run: RunId) -> Result<Option<Checkpoint>, Self::Error>;

  /// Interrupted runs of the project, ordered by number, for recovery.
  fn interrupted_runs(&self) -> Result<Vec<Run>, Self::Error>;
}
