//! What a shared project's web editor can pick (`docs/architecture/server.md`, "What the web editor can pick"): this
//! computer's report of its CLI kinds and models, and the two reads of the union, which members and admins make alike
//! and the web app's server side makes with its browser sign-in.

use orchester_core::{id::ProjectId, secret::SecretValue};
use reqwest::Method;

use super::{Client, ClientError, bearer};
use crate::{CatalogCliKinds, CatalogModels, CatalogReport, ModelsPerRole};

impl Client {
  /// `PUT …/catalog`: this computer's whole state, which replaces its device's previous report (`204`); `422 invalid
  /// { reason: "catalog" }` for a CLI kind or role that is not an agent role or a report over the limits
  /// ([`CatalogReport::check`]).
  pub async fn report_catalog(
    &self,
    token: &SecretValue,
    project: ProjectId,
    report: &CatalogReport,
  ) -> Result<(), ClientError> {
    let path = format!("/projects/{project}/catalog");
    self.send(bearer(self.json(Method::PUT, &path, report)?, token)).await
  }

  /// `GET …/cli-kinds`: the CLI kinds the members reported, sorted, once each.
  pub async fn catalog_cli_kinds(&self, token: &SecretValue, project: ProjectId) -> Result<Vec<String>, ClientError> {
    let path = format!("/projects/{project}/cli-kinds");
    let response: CatalogCliKinds = self.call(bearer(self.request(Method::GET, &path)?, token)).await?;
    Ok(response.cli_kinds)
  }

  /// `GET …/models`: the model ids the members reported, per connection role.
  pub async fn catalog_models(&self, token: &SecretValue, project: ProjectId) -> Result<ModelsPerRole, ClientError> {
    let path = format!("/projects/{project}/models");
    let response: CatalogModels = self.call(bearer(self.request(Method::GET, &path)?, token)).await?;
    Ok(response.models)
  }
}
