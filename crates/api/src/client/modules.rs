//! A shared project's modules (`docs/architecture/server.md`, "Modules"): reading them with the moves of tasks into and
//! out of them, making one and changing one under its revision. A task's module changes with the task
//! ([`Client::update_task`]).

use orchester_core::{
  id::{ModuleId, ProjectId},
  secret::SecretValue,
};
use reqwest::Method;

use super::{Client, ClientError, bearer};
use crate::{ModuleCreate, ModulePatch, ModuleResponse, Modules, SharedModule};

impl Client {
  /// `GET …/modules?after=`: every module of the project, and the moves after the sequence `after`.
  pub async fn modules(&self, token: &SecretValue, project: ProjectId, after: u64) -> Result<Modules, ClientError> {
    let mut url = self.address.endpoint(&format!("/projects/{project}/modules"))?;
    url.query_pairs_mut().append_pair("after", &after.to_string());
    self.call(bearer(self.http.get(url), token)).await
  }

  /// `POST …/modules`: a module; `409 module_exists` names the module that has its name. A repeated key returns the
  /// first answer.
  pub async fn create_module(
    &self,
    token: &SecretValue,
    project: ProjectId,
    create: &ModuleCreate,
  ) -> Result<SharedModule, ClientError> {
    let path = format!("/projects/{project}/modules");
    let response: ModuleResponse = self
      .call(bearer(self.json(Method::POST, &path, create)?, token))
      .await?;
    Ok(response.module)
  }

  /// `PATCH …/modules/{module}`: the module's fields, made on the revision read; `409 revision_conflict` carries the
  /// module as it is now.
  pub async fn update_module(
    &self,
    token: &SecretValue,
    project: ProjectId,
    module: ModuleId,
    patch: &ModulePatch,
  ) -> Result<SharedModule, ClientError> {
    let path = format!("/projects/{project}/modules/{module}");
    let response: ModuleResponse = self
      .call(bearer(self.json(Method::PATCH, &path, patch)?, token))
      .await?;
    Ok(response.module)
  }
}
