//! The S10 bounded reads of one team project, on the team server's routes (its `docs/workspace-reads.md`): the
//! workspace and the caller's capabilities, the task list with its filters, search, sort and board counts, a task, the
//! modules with their progress, burndown and history, the resource library, its pickers and a resource's versions.
//! Each listing answers one [`Page`] at a time. A version's body comes in its page of the version list; there is no
//! route of its own for one version.

use orchester_core::id::{ModuleId, ResourceId, TaskId};

use super::{Authority, AuthorityCallError};
use crate::v2::{
  Capabilities, ModuleBurndownRead, ModuleHistoryPage, ModuleListRequest, ModuleProgressRead, ModuleSummary, Page,
  PageRequest, ResourceLibraryPage, ResourceLibraryRequest, ResourcePickerPage, ResourcePickerRequest,
  ResourceVersionsPage, TaskBoardCounts, TaskFilters, TaskListRequest, TaskRead, WorkspaceMetadata,
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

  /// `POST …/task-list`: one page of the tasks `request`'s filters keep, in its order.
  pub async fn tasks(&self, request: &TaskListRequest) -> Result<Page<TaskRead>, AuthorityCallError> {
    self.query("/task-list", request).await
  }

  /// `POST …/task-counts`: how many of the tasks `filters` keep are in each workflow status.
  pub async fn task_counts(&self, filters: &TaskFilters) -> Result<TaskBoardCounts, AuthorityCallError> {
    self.query("/task-counts", filters).await
  }

  /// `GET …/tasks/{task}`.
  pub async fn task(&self, task: TaskId) -> Result<TaskRead, AuthorityCallError> {
    self.get(&format!("/tasks/{task}")).await
  }

  /// `POST …/module-list`: one page of the modules with their progress and forecast, oldest first.
  pub async fn modules(&self, request: &ModuleListRequest) -> Result<Page<ModuleSummary>, AuthorityCallError> {
    self.query("/module-list", request).await
  }

  /// `GET …/modules/{module}`: the module with its progress and forecast.
  pub async fn module(&self, module: ModuleId) -> Result<ModuleSummary, AuthorityCallError> {
    self.get(&format!("/modules/{module}")).await
  }

  /// `GET …/modules/{module}/progress`.
  pub async fn progress(&self, module: ModuleId) -> Result<ModuleProgressRead, AuthorityCallError> {
    self.get(&format!("/modules/{module}/progress")).await
  }

  /// `GET …/modules/{module}/burndown`: the burndown by day with the forecast.
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

  /// `POST …/library`: one page of the library, without the resources' versions.
  pub async fn library(&self, request: &ResourceLibraryRequest) -> Result<ResourceLibraryPage, AuthorityCallError> {
    self.query("/library", request).await
  }

  /// `POST …/resource-picker`: one page of the resources a form may choose.
  pub async fn picker(&self, request: &ResourcePickerRequest) -> Result<ResourcePickerPage, AuthorityCallError> {
    self.query("/resource-picker", request).await
  }

  /// `POST …/resources/{resource}/version-list`: one page of the resource's published versions with their bodies,
  /// oldest first.
  pub async fn versions(
    &self,
    resource: ResourceId,
    page: &PageRequest,
  ) -> Result<ResourceVersionsPage, AuthorityCallError> {
    self.query(&format!("/resources/{resource}/version-list"), page).await
  }
}
