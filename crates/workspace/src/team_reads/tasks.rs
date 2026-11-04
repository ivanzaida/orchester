//! A team project's tasks as the server reads them (S10): a page of the list with its filters, search and order, the
//! board's counts by workflow status for the same filters, and one task. The list and its counts belong to
//! [`QueryScope::TaskList`], a task to [`QueryScope::Task`].

use orchester_api::v2::{Page, QueryScope, TaskBoardCounts, TaskFilters, TaskListRequest, TaskRead};
use orchester_core::id::TaskId;

use super::{TeamProject, TeamRead, TeamReadError};

impl TeamProject {
  /// One page of the tasks `request` keeps, at most its page limit. The pages after it carry the same snapshot; a
  /// page cursor whose snapshot expired is [`TeamReadError::CursorExpired`].
  pub async fn task_page(&self, request: TaskListRequest) -> Result<TeamRead<Page<TaskRead>>, TeamReadError> {
    self
      .read(QueryScope::TaskList {}, |inner| {
        let request = request.clone();
        async move { inner.session.authority().tasks(&request).await }
      })
      .await
  }

  /// How many of the tasks `filters` keep are in each workflow status.
  pub async fn board_counts(&self, filters: TaskFilters) -> Result<TeamRead<TaskBoardCounts>, TeamReadError> {
    self
      .read(QueryScope::TaskList {}, |inner| {
        let filters = filters.clone();
        async move { inner.session.authority().task_counts(&filters).await }
      })
      .await
  }

  pub async fn task(&self, task: TaskId) -> Result<TeamRead<TaskRead>, TeamReadError> {
    self
      .read(QueryScope::Task { task }, |inner| async move {
        inner.session.authority().task(task).await
      })
      .await
  }
}
