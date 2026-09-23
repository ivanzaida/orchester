//! Persistence boundaries. The local SQLite store implements them today; a server client
//! will implement the same traits later, so domain code depends only on these.

use crate::{
  id::{IdempotencyKey, TaskId},
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
