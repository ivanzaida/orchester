//! Persistence boundaries. The local SQLite store implements them today; a server client
//! will implement the same traits later, so domain code depends only on these.

use crate::{
  id::{IdempotencyKey, ResourceId, TaskId},
  name::Name,
  resource::{Draft, Resource, ResourceBody, ResourceKind, Slug, Version, VersionNumber},
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
