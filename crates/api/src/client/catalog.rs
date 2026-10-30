//! What a shared project's web editor can pick (`docs/architecture/server.md`, "What the web editor can pick"): this
//! computer's report of its connections, MCP servers and models, and the two reads of the union, which members and
//! admins make alike and the web app's server side makes with its browser sign-in.

use orchester_core::{id::ProjectId, secret::SecretValue};
use reqwest::Method;

use super::{Client, ClientError, bearer};
use crate::{CatalogModels, CatalogReference, CatalogReferences, CatalogReport, ModelsPerRole};

impl Client {
  /// `PUT …/catalog`: this computer's whole state, which replaces its device's previous report (`204`); `422 invalid
  /// { reason: "catalog" }` for an unknown kind or role or a report over the limits.
  pub async fn report_catalog(
    &self,
    token: &SecretValue,
    project: ProjectId,
    report: &CatalogReport,
  ) -> Result<(), ClientError> {
    let path = format!("/projects/{project}/catalog");
    self.send(bearer(self.json(Method::PUT, &path, report)?, token)).await
  }

  /// `GET …/references`: the connections and MCP servers the members reported and the project's versions name, once
  /// per kind, role and name.
  pub async fn catalog_references(
    &self,
    token: &SecretValue,
    project: ProjectId,
  ) -> Result<Vec<CatalogReference>, ClientError> {
    let path = format!("/projects/{project}/references");
    let response: CatalogReferences = self.call(bearer(self.request(Method::GET, &path)?, token)).await?;
    Ok(response.references)
  }

  /// `GET …/models`: the model ids the members reported, per connection role.
  pub async fn catalog_models(&self, token: &SecretValue, project: ProjectId) -> Result<ModelsPerRole, ClientError> {
    let path = format!("/projects/{project}/models");
    let response: CatalogModels = self.call(bearer(self.request(Method::GET, &path)?, token)).await?;
    Ok(response.models)
  }
}
