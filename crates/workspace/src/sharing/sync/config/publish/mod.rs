//! `RemoteResourceStore`, a shared project's [`ResourceStore`] (`docs/architecture/server.md`, "The desktop's trait
//! boundary", "Shared configuration"): it reads the cache, keeps an admin's drafts on this computer, and publishes
//! through the versions waiting for the server ([`send`]). A version published while the server cannot be reached is
//! used by the admin's own new runs at once and sent on reconnect; one that meets a version someone published from the
//! same base meanwhile waits for the admin to decide ([`RemoteResourceStore::publish_again`]). A member has no drafts
//! of the team's resources: this computer and the server refuse them alike ([`SharingError::NotAdmin`]).
//! `ResourceStore` is synchronous: the store waits on a worker of the application's runtime for each request.

mod send;

use orchester_api::{
  FileStored, ResourceCreate, ResourceCreated, ResourcesImport, SharedResource, SharedVersion, VersionPublish, Versions,
};
use orchester_core::{
  blob::ContentHash,
  id::{ResourceId, VersionId},
  name::Name,
  resource::{
    Draft, DraftSaved, Removal, RemoveMode, Resource, ResourceBody, ResourceKind, Slug, Version, VersionNumber,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, ResourceSharing, StoreError};
use tokio::runtime::Handle;

use crate::sharing::SharingError;

/// A shared project's server as the team's configuration is published to it, signed in as the person.
pub trait ConfigServer: Send + Sync {
  /// Fails at once while the server cannot take a change.
  fn writable(&self) -> Result<(), SharingError>;

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

pub struct RemoteResourceStore<S> {
  db: ProjectDb,
  server: S,
  runtime: Handle,
}

impl<S: ConfigServer> RemoteResourceStore<S> {
  /// The store of the shared project `server` is the server of, over its database `db`. Must be made on a worker of
  /// the application's runtime, whose requests it waits for.
  pub fn new(db: ProjectDb, server: S) -> Result<Self, SharingError> {
    let runtime = Handle::try_current().map_err(|_| SharingError::Interrupted)?;
    Ok(Self { db, server, runtime })
  }

  /// The cache, for reads.
  pub const fn db(&self) -> &ProjectDb {
    &self.db
  }

  /// The admin publishes `version`, which met a version someone published from the same base, as the next version
  /// anyway: it is sent again at once, based on the latest version this computer has of the resource.
  pub fn publish_again(&mut self, version: VersionId) -> Result<(), SharingError> {
    let waiting = self
      .db
      .waiting_publishes()?
      .into_iter()
      .find(|waiting| waiting.version == version)
      .ok_or(StoreError::Corrupt {
        what: "waiting publish",
        detail: version.to_string(),
      })?;
    if waiting.kind == ResourceKind::Skill {
      // A teammate's skill version is not kept here yet, so this computer has no newer base to send it on.
      return Err(StoreError::SharedProject.into());
    }
    self.db.publish_again(version)?;
    if self.server.writable().is_err() {
      return Ok(());
    }
    self.send_resource(waiting.resource)
  }

  /// `version` of `resource` as it is kept now: the server may have given it another number.
  fn kept<B: ResourceBody>(&self, resource: ResourceId, version: VersionId) -> Result<Version<B>, SharingError> {
    self
      .db
      .versions::<B>(resource)?
      .into_iter()
      .find(|kept| kept.id() == version)
      .ok_or_else(|| StoreError::ResourceNotFound(resource).into())
  }
}

/// A refusal of this computer's, said as the server's would be.
fn local(error: StoreError) -> SharingError {
  match error {
    StoreError::NotAdmin => SharingError::NotAdmin,
    error => error.into(),
  }
}

impl<S: ConfigServer> ResourceStore for RemoteResourceStore<S> {
  type Error = SharingError;

  /// A new resource of the team's, kept here as a draft until the admin publishes it: its first version creates it on
  /// the server.
  fn create_resource<B: ResourceBody>(
    &mut self,
    slug: Slug,
    name: Name,
    body: &B,
    now: Timestamp,
  ) -> Result<Resource, SharingError> {
    self.db.create_resource(slug, name, body, now).map_err(local)
  }

  fn resource(&self, id: ResourceId) -> Result<Option<Resource>, SharingError> {
    Ok(self.db.resource(id)?)
  }

  fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, SharingError> {
    Ok(self.db.resources(kind)?)
  }

  fn slugs(&self, kind: ResourceKind) -> Result<Vec<Slug>, SharingError> {
    Ok(self.db.slugs(kind)?)
  }

  /// A resource the server has is archived there, which this store does not send yet: it is refused, not removed here
  /// alone.
  fn remove_resource(&mut self, id: ResourceId, mode: RemoveMode, now: Timestamp) -> Result<Removal, SharingError> {
    if !self.db.may_change_configuration()? {
      return Err(SharingError::NotAdmin);
    }
    if matches!(self.db.resource_sharing(id)?, Some(ResourceSharing::Shared(_))) {
      return Err(StoreError::SharedProject.into());
    }
    self.db.remove_resource(id, mode, now).map_err(local)
  }

  fn save_draft<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    draft: &Draft<B>,
  ) -> Result<DraftSaved, SharingError> {
    self.db.save_draft(resource, draft).map_err(local)
  }

  /// A member has no drafts of the team's resources.
  fn draft<B: ResourceBody>(&self, resource: ResourceId) -> Result<Option<Draft<B>>, SharingError> {
    if !self.db.may_change_configuration()? {
      return Ok(None);
    }
    Ok(self.db.draft(resource)?)
  }

  /// Publishes the draft here at once, where the admin's new runs use it, and sends it when the server can be reached:
  /// the version comes back with the server's number, or, offline, with its number here until it is sent. A version
  /// someone published from the same base meanwhile is [`SharingError::VersionConflict`]; the version stays published
  /// here, waiting for the admin's decision.
  fn publish_draft<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    now: Timestamp,
  ) -> Result<Version<B>, SharingError> {
    let published = self.db.publish_draft::<B>(resource, now).map_err(local)?;
    if self.server.writable().is_ok() {
      self.send_resource(resource)?;
    }
    self.kept(resource, published.id())
  }

  fn latest_version<B: ResourceBody>(&self, resource: ResourceId) -> Result<Option<Version<B>>, SharingError> {
    Ok(self.db.latest_version(resource)?)
  }

  fn versions<B: ResourceBody>(&self, resource: ResourceId) -> Result<Vec<Version<B>>, SharingError> {
    Ok(self.db.versions(resource)?)
  }

  fn version<B: ResourceBody>(
    &self,
    resource: ResourceId,
    number: VersionNumber,
  ) -> Result<Option<Version<B>>, SharingError> {
    Ok(self.db.version(resource, number)?)
  }
}
