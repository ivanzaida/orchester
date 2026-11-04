//! A team project's modules as the server reads them (S10), counted on the server by core's rules: a page of the
//! modules with their progress and forecast, one module, its burndown and a page of its history. The list belongs to
//! [`QueryScope::ModuleList`], the rest to the module's [`QueryScope::Module`]. A summary's progress converts to core's
//! [`orchester_core::module::progress::Progress`] (`ModuleProgressRead::to_core`), whose percent and remaining work the
//! screens show.

use orchester_api::v2::{
  ModuleBurndownRead, ModuleHistoryPage, ModuleListRequest, ModuleSummary, Page, PageRequest, QueryScope,
};
use orchester_core::id::ModuleId;

use super::{TeamProject, TeamRead, TeamReadError};

impl TeamProject {
  /// One page of the modules `request` asks for.
  pub async fn module_page(&self, request: ModuleListRequest) -> Result<TeamRead<Page<ModuleSummary>>, TeamReadError> {
    self
      .read(QueryScope::ModuleList {}, |inner| {
        let request = request.clone();
        async move { inner.session.authority().modules(&request).await }
      })
      .await
  }

  pub async fn module(&self, module: ModuleId) -> Result<TeamRead<ModuleSummary>, TeamReadError> {
    self
      .read(QueryScope::Module { module }, |inner| async move {
        inner.session.authority().module(module).await
      })
      .await
  }

  /// The module's burndown by day, at most a year of days, with its forecast.
  pub async fn burndown(&self, module: ModuleId) -> Result<TeamRead<ModuleBurndownRead>, TeamReadError> {
    self
      .read(QueryScope::Module { module }, |inner| async move {
        inner.session.authority().burndown(module).await
      })
      .await
  }

  /// One page of the moves of tasks into and out of the module, in ascending sequence.
  pub async fn module_history(
    &self,
    module: ModuleId,
    page: PageRequest,
  ) -> Result<TeamRead<ModuleHistoryPage>, TeamReadError> {
    self
      .read(QueryScope::Module { module }, |inner| {
        let page = page.clone();
        async move { inner.session.authority().module_history(module, &page).await }
      })
      .await
  }
}
