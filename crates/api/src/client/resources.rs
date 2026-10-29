//! A shared project's configuration (`docs/architecture/server.md`, "Shared configuration", "Resources"): reading its
//! resources and their versions, and, for admins only (`403 not_admin` for a member), creating, importing, publishing,
//! renaming and archiving them and uploading a skill's parts by their hash.

use orchester_core::{
  blob::ContentHash,
  id::{ProjectId, ResourceId},
  resource::VersionNumber,
  secret::SecretValue,
};
use reqwest::{Method, header::CONTENT_TYPE};

use super::{Client, ClientError, answered, bearer};
use crate::{
  FileStored, ResourceCreate, ResourceCreated, ResourcePatch, ResourceResponse, Resources, ResourcesImport,
  ResourcesImported, SharedResource, SharedVersion, VersionPublish, VersionResponse, Versions,
};

impl Client {
  /// `GET …/resources`: every resource of the project and the event cursor the list is current at.
  pub async fn resources(&self, token: &SecretValue, project: ProjectId) -> Result<Resources, ClientError> {
    let path = format!("/projects/{project}/resources");
    self.call(bearer(self.request(Method::GET, &path)?, token)).await
  }

  /// `GET …/resources/{resource}/versions?after=`: the versions after `after` (from v1 without it), oldest first, one
  /// page at a time; `next` asks for the following page.
  pub async fn versions(
    &self,
    token: &SecretValue,
    project: ProjectId,
    resource: ResourceId,
    after: Option<VersionNumber>,
  ) -> Result<Versions, ClientError> {
    let mut url = self
      .address
      .endpoint(&format!("/projects/{project}/resources/{resource}/versions"))?;
    if let Some(after) = after {
      url.query_pairs_mut().append_pair("after", &after.get().to_string());
    }
    self.call(bearer(self.http.get(url), token)).await
  }

  /// `POST …/resources`: a resource and its v1; `409 slug_taken` names the resource of its kind that has the slug. A
  /// repeated key returns the first answer.
  pub async fn create_resource(
    &self,
    token: &SecretValue,
    project: ProjectId,
    create: &ResourceCreate,
  ) -> Result<ResourceCreated, ClientError> {
    let path = format!("/projects/{project}/resources");
    self.call(bearer(self.json(Method::POST, &path, create)?, token)).await
  }

  /// `POST …/resources/import`: local resources uploaded with their history, ids and version numbers kept.
  pub async fn import_resources(
    &self,
    token: &SecretValue,
    project: ProjectId,
    import: &ResourcesImport,
  ) -> Result<Vec<SharedResource>, ClientError> {
    let path = format!("/projects/{project}/resources/import");
    let response: ResourcesImported = self
      .call(bearer(self.json(Method::POST, &path, import)?, token))
      .await?;
    Ok(response.resources)
  }

  /// `POST …/resources/{resource}/versions`: publishes the next version; `409 version_conflict` carries the latest
  /// when someone published from the same base meanwhile, `422 missing_files` the parts to upload first.
  pub async fn publish_version(
    &self,
    token: &SecretValue,
    project: ProjectId,
    resource: ResourceId,
    publish: &VersionPublish,
  ) -> Result<SharedVersion, ClientError> {
    let path = format!("/projects/{project}/resources/{resource}/versions");
    let response: VersionResponse = self
      .call(bearer(self.json(Method::POST, &path, publish)?, token))
      .await?;
    Ok(response.version)
  }

  /// `PATCH …/resources/{resource}`: renames or archives the resource, made on the revision read; `409
  /// revision_conflict` carries the resource as it is now.
  pub async fn update_resource(
    &self,
    token: &SecretValue,
    project: ProjectId,
    resource: ResourceId,
    patch: &ResourcePatch,
  ) -> Result<SharedResource, ClientError> {
    let path = format!("/projects/{project}/resources/{resource}");
    let response: ResourceResponse = self
      .call(bearer(self.json(Method::PATCH, &path, patch)?, token))
      .await?;
    Ok(response.resource)
  }

  /// `PUT …/files/{sha256}`: one skill part, at most 1 MiB (`413 payload_too_large` over it), whose content must be
  /// `hash` (`422 hash_mismatch`). Uploading it again changes nothing.
  pub async fn upload_file(
    &self,
    token: &SecretValue,
    project: ProjectId,
    hash: &ContentHash,
    content: Vec<u8>,
  ) -> Result<FileStored, ClientError> {
    let path = format!("/projects/{project}/files/{hash}");
    let request = self
      .request(Method::PUT, &path)?
      .header(CONTENT_TYPE, "application/octet-stream")
      .body(content);
    let response = answered(bearer(request, token)).await?;
    match response.status().as_u16() {
      201 => Ok(FileStored::Created),
      200 => Ok(FileStored::Known),
      status => Err(ClientError::NotOrchester { status }),
    }
  }

  /// `GET …/files/{sha256}`: the content of one skill part; `404` when the project does not have it.
  pub async fn file(
    &self,
    token: &SecretValue,
    project: ProjectId,
    hash: &ContentHash,
  ) -> Result<Vec<u8>, ClientError> {
    let path = format!("/projects/{project}/files/{hash}");
    let response = answered(bearer(self.request(Method::GET, &path)?, token)).await?;
    let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
    Ok(bytes.to_vec())
  }
}
