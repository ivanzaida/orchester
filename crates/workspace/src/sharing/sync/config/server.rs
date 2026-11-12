//! A shared project's server as the team's configuration is sent to it, signed in as the person ([`ConfigServer`]):
//! Share's upload, and an admin's change sent straight to the server (`crate::sharing::team_edit`).

use orchester_api::{
  FileStored, ResourceCreate, ResourceCreated, ResourcesImport, SharedResource, SharedVersion, VersionPublish, Versions,
};
use orchester_core::{blob::ContentHash, id::ResourceId, resource::VersionNumber};

use crate::sharing::SharingError;

/// A shared project's server as the team's configuration is sent to it, signed in as the person.
pub trait ConfigServer: Send + Sync {
  /// Reads one page of `resource`'s versions after `after`, from v1 without it.
  fn versions(
    &self,
    resource: ResourceId,
    after: Option<VersionNumber>,
  ) -> impl Future<Output = Result<Versions, SharingError>> + Send;

  /// Creates a resource with its first version.
  fn create_resource(
    &self,
    create: ResourceCreate,
  ) -> impl Future<Output = Result<ResourceCreated, SharingError>> + Send;

  /// Publishes the next version of `resource`.
  fn publish_version(
    &self,
    resource: ResourceId,
    publish: VersionPublish,
  ) -> impl Future<Output = Result<SharedVersion, SharingError>> + Send;

  /// Uploads one skill part by its hash.
  fn upload_file(
    &self,
    hash: ContentHash,
    content: Vec<u8>,
  ) -> impl Future<Output = Result<FileStored, SharingError>> + Send;

  /// Uploads local resources with their history, ids and version numbers kept (Share).
  fn import_resources(
    &self,
    import: ResourcesImport,
  ) -> impl Future<Output = Result<Vec<SharedResource>, SharingError>> + Send;
}
