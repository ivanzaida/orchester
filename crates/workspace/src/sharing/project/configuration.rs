//! A shared project's server as the team's configuration is published to it ([`ConfigServer`]), through the stored
//! trust and token of the account signed in to it.

use orchester_api::{
  FileStored, ResourceCreate, ResourceCreated, ResourcesImport, SharedResource, SharedVersion, VersionPublish, Versions,
};
use orchester_core::{blob::ContentHash, id::ResourceId, resource::VersionNumber};

use super::ProjectServer;
use crate::sharing::{SharingError, sync::ConfigServer};

impl ConfigServer for ProjectServer {
  fn writable(&self) -> Result<(), SharingError> {
    Self::writable(self)
  }

  async fn versions(&self, resource: ResourceId, after: Option<VersionNumber>) -> Result<Versions, SharingError> {
    self
      .call(|client, token, project| async move { client.versions(&token, project, resource, after).await })
      .await
  }

  async fn create_resource(&self, create: ResourceCreate) -> Result<ResourceCreated, SharingError> {
    self
      .call(|client, token, project| async move { client.create_resource(&token, project, &create).await })
      .await
  }

  async fn publish_version(
    &self,
    resource: ResourceId,
    publish: VersionPublish,
  ) -> Result<SharedVersion, SharingError> {
    self
      .call(|client, token, project| async move { client.publish_version(&token, project, resource, &publish).await })
      .await
  }

  async fn upload_file(&self, hash: ContentHash, content: Vec<u8>) -> Result<FileStored, SharingError> {
    self
      .call(|client, token, project| async move { client.upload_file(&token, project, &hash, content).await })
      .await
  }

  async fn import_resources(&self, import: ResourcesImport) -> Result<Vec<SharedResource>, SharingError> {
    self
      .call(|client, token, project| async move { client.import_resources(&token, project, &import).await })
      .await
  }
}
