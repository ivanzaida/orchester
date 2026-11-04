//! The S10 bounded reads of one team project: the workspace and the caller's capabilities, the task list with its
//! filters, search, sort and board counts, a task, the modules with their progress, burndown and history, the resource
//! library, its pickers and a resource's versions. Each listing answers one [`Page`] at a time.

use orchester_core::{
  id::{ModuleId, ResourceId, TaskId},
  resource::VersionNumber,
};

use super::{Authority, AuthorityCallError};
use crate::{
  SharedVersion,
  v2::{
    Capabilities, ModuleBurndownRead, ModuleHistoryPage, ModuleListRequest, ModuleSummary, Page, PageRequest,
    ResourceLibraryPage, ResourceLibraryRequest, ResourcePickerPage, ResourcePickerRequest, ResourceVersionsPage,
    TaskBoardCounts, TaskFilters, TaskListRequest, TaskRead, WorkspaceMetadata,
  },
};

impl Authority<'_> {
  /// `GET …/workspace`: the project, its policy and the caller's capabilities.
  pub async fn workspace(&self) -> Result<WorkspaceMetadata, AuthorityCallError> {
    self.get("/workspace").await
  }

  /// `GET …/capabilities`: what the caller may do in the workspace.
  pub async fn capabilities(&self) -> Result<Capabilities, AuthorityCallError> {
    self.get("/capabilities").await
  }

  /// `POST …/tasks/query`: one page of the tasks `request`'s filters keep, in its order.
  pub async fn tasks(&self, request: &TaskListRequest) -> Result<Page<TaskRead>, AuthorityCallError> {
    self.query("/tasks/query", request).await
  }

  /// `POST …/tasks/counts`: how many of the tasks `filters` keep are in each workflow status.
  pub async fn task_counts(&self, filters: &TaskFilters) -> Result<TaskBoardCounts, AuthorityCallError> {
    self.query("/tasks/counts", filters).await
  }

  /// `GET …/tasks/{task}`.
  pub async fn task(&self, task: TaskId) -> Result<TaskRead, AuthorityCallError> {
    self.get(&format!("/tasks/{task}")).await
  }

  /// `POST …/modules/query`: one page of the modules with their progress and forecast.
  pub async fn modules(&self, request: &ModuleListRequest) -> Result<Page<ModuleSummary>, AuthorityCallError> {
    self.query("/modules/query", request).await
  }

  /// `GET …/modules/{module}`: the module with its progress and forecast.
  pub async fn module(&self, module: ModuleId) -> Result<ModuleSummary, AuthorityCallError> {
    self.get(&format!("/modules/{module}")).await
  }

  /// `GET …/modules/{module}/burndown`.
  pub async fn burndown(&self, module: ModuleId) -> Result<ModuleBurndownRead, AuthorityCallError> {
    self.get(&format!("/modules/{module}/burndown")).await
  }

  /// `POST …/modules/{module}/history`: one page of the moves of tasks into and out of the module.
  pub async fn module_history(
    &self,
    module: ModuleId,
    page: &PageRequest,
  ) -> Result<ModuleHistoryPage, AuthorityCallError> {
    self.query(&format!("/modules/{module}/history"), page).await
  }

  /// `POST …/resources/query`: one page of the library.
  pub async fn library(&self, request: &ResourceLibraryRequest) -> Result<ResourceLibraryPage, AuthorityCallError> {
    self.query("/resources/query", request).await
  }

  /// `POST …/resources/picker`: one page of the resources a form may choose.
  pub async fn picker(&self, request: &ResourcePickerRequest) -> Result<ResourcePickerPage, AuthorityCallError> {
    self.query("/resources/picker", request).await
  }

  /// `POST …/resources/{resource}/versions/query`: one page of the resource's published versions, oldest first.
  pub async fn versions(
    &self,
    resource: ResourceId,
    page: &PageRequest,
  ) -> Result<ResourceVersionsPage, AuthorityCallError> {
    self.query(&format!("/resources/{resource}/versions/query"), page).await
  }

  /// `GET …/resources/{resource}/versions/{version}`: one published version with its body.
  pub async fn version(
    &self,
    resource: ResourceId,
    version: VersionNumber,
  ) -> Result<SharedVersion, AuthorityCallError> {
    self
      .get(&format!("/resources/{resource}/versions/{}", version.get()))
      .await
  }
}
