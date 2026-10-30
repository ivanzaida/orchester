//! Sending the versions an admin published on this computer (`docs/architecture/server.md`, "Shared configuration",
//! row 36): each resource's in the order they were published, each based on the version it was edited from, with
//! the references of what its body names that is not shared. A resource's first version creates it on the server. A
//! skill's parts the server does not have yet are uploaded by their hash, then the version is sent again. The server's
//! number replaces the one the version had here; a version someone published from the same base meanwhile is cached
//! with those before it, and the version waits for the admin's decision, as one the server refused does; the
//! resource's later versions wait behind it.

use orchester_api::{
  ErrorCode, ResourceCreate, SharedResource, SharedVersion, SharingErrorDetails, VersionContent, VersionPublish,
};
use orchester_core::{
  blob::ContentHash,
  id::ResourceId,
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Skill, Tool},
  secret::SecretKind,
  sharing::{ConnectionRole, LocalEntities, Published, Reference, for_publishing},
  store::{BlobStore, ConnectionStore, McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{
  CachedVersion, PublishState, ReferenceTarget, ResourceSharing, StoreError, VersionReference, WaitingPublish,
};

use super::{ConfigServer, RemoteResourceStore};
use crate::sharing::{
  SharingError,
  project::convert,
  sync::config::{bindings, cache_version, reference, resource as kept_resource, shared_reference},
};

impl<S: ConfigServer> RemoteResourceStore<S> {
  /// Sends every version published here that waits, e.g. once the server can be reached again. One the server did not
  /// take is recorded on it and logged; the others are still sent. Fails, sending nothing more, when the server cannot
  /// be reached.
  pub fn send_waiting(&mut self) -> Result<(), SharingError> {
    let mut resources: Vec<ResourceId> = Vec::new();
    for waiting in self.db.waiting_publishes()? {
      if waiting.state == PublishState::Waiting && !resources.contains(&waiting.resource) {
        resources.push(waiting.resource);
      }
    }
    if resources.is_empty() {
      return Ok(());
    }
    self.server.writable()?;
    for resource in resources {
      match self.send_resource(resource) {
        Ok(()) => {}
        Err(error) if unreached(&error) => return Err(error),
        Err(error) => tracing::warn!(%error, %resource, "the server did not take a version published here"),
      }
    }
    Ok(())
  }

  /// Sends the waiting versions of `resource`, oldest first, until one is not taken.
  pub(super) fn send_resource(&mut self, resource: ResourceId) -> Result<(), SharingError> {
    loop {
      let next = self
        .db
        .waiting_publishes()?
        .into_iter()
        .find(|waiting| waiting.resource == resource);
      match next {
        Some(waiting) if waiting.state == PublishState::Waiting => self.send_one(&waiting)?,
        // Later versions wait behind one the admin decides on.
        _ => return Ok(()),
      }
    }
  }

  fn send_one(&mut self, waiting: &WaitingPublish) -> Result<(), SharingError> {
    match waiting.kind {
      ResourceKind::Pipeline => self.send_kind::<Pipeline>(waiting),
      ResourceKind::AgentProfile => self.send_kind::<AgentProfile>(waiting),
      ResourceKind::Instruction => self.send_kind::<Instruction>(waiting),
      ResourceKind::Skill => self.send_kind::<Skill>(waiting),
      ResourceKind::Tool => self.send_kind::<Tool>(waiting),
      // The store never records a version of agent testing's kinds, which stay on this computer.
      ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => Err(
        StoreError::Corrupt {
          what: "waiting publish",
          detail: waiting.version.to_string(),
        }
        .into(),
      ),
    }
  }

  fn send_kind<B: ResourceBody + Clone>(&mut self, waiting: &WaitingPublish) -> Result<(), SharingError> {
    let published = self.publication::<B>(waiting)?;
    match self.deliver(waiting, &published) {
      Ok((resource, version)) => self.confirm::<B>(waiting, resource.as_ref(), &version),
      Err(SharingError::Client(error)) => match (error.code(), error.details()) {
        (Some(ErrorCode::VersionConflict), Some(SharingErrorDetails::Version { latest })) => {
          let latest = latest.number;
          self.catch_up(waiting.resource, waiting.kind)?;
          self.db.record_publish_conflict(waiting.version, latest)?;
          Err(SharingError::VersionConflict { latest })
        }
        (Some(code), _) => {
          self.db.record_publish_refused(waiting.version, &code_text(code))?;
          Err(match code {
            ErrorCode::NotAdmin => SharingError::NotAdmin,
            _ => SharingError::Client(error),
          })
        }
        (None, _) => Err(SharingError::Client(error)),
      },
      Err(error) => Err(error),
    }
  }

  /// `waiting`'s body in the publisher's ids, with a reference for each connection, MCP server and secret it names.
  fn publication<B: ResourceBody>(&self, waiting: &WaitingPublish) -> Result<Published, SharingError> {
    let version = self
      .db
      .version::<B>(waiting.resource, waiting.number)?
      .ok_or(StoreError::ResourceNotFound(waiting.resource))?;
    let body = serde_json::to_value(version.body()).map_err(|error| StoreError::Corrupt {
      what: "resource body",
      detail: error.to_string(),
    })?;
    let base = match waiting.based_on {
      Some(number) => self.db.cached_version::<B>(waiting.resource, number)?,
      None => None,
    };
    let base: Vec<Reference> = base
      .map(|base| base.references.iter().filter_map(base_reference).collect())
      .unwrap_or_default();
    let (connections, mcp_servers, secrets) = (self.db.connections()?, self.db.mcp_servers()?, self.db.secrets()?);
    let entities = LocalEntities {
      connections: &connections,
      mcp_servers: &mcp_servers,
      secrets: &secrets,
    };
    let bound = bindings::bound(&self.db.reference_bindings()?);
    Ok(for_publishing(body, entities, &base, &bound))
  }

  /// Sends `waiting` as `published`, uploading the skill parts the server asks for first; the resource when the
  /// version created it, and the version as the server numbered it.
  fn deliver(
    &self,
    waiting: &WaitingPublish,
    published: &Published,
  ) -> Result<(Option<SharedResource>, SharedVersion), SharingError> {
    match self.attempt(waiting, published) {
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::MissingFiles) => {
        let Some(SharingErrorDetails::Files { hashes }) = error.details() else {
          return Err(SharingError::InvalidAnswer("missing files"));
        };
        for hash in hashes.clone() {
          self.upload(hash)?;
        }
        self.attempt(waiting, published)
      }
      answer => answer,
    }
  }

  fn attempt(
    &self,
    waiting: &WaitingPublish,
    published: &Published,
  ) -> Result<(Option<SharedResource>, SharedVersion), SharingError> {
    let content = VersionContent {
      body: published.body.clone(),
      references: published.references.iter().map(shared_reference).collect(),
    };
    let on_server = matches!(
      self.db.resource_sharing(waiting.resource)?,
      Some(ResourceSharing::Shared(_))
    );
    if waiting.based_on.is_none() && !on_server {
      let resource = self
        .db
        .resource(waiting.resource)?
        .ok_or(StoreError::ResourceNotFound(waiting.resource))?;
      let kind = orchester_api::SharedResourceKind::from_core(resource.kind()).ok_or(StoreError::Corrupt {
        what: "waiting publish",
        detail: waiting.version.to_string(),
      })?;
      let create = ResourceCreate {
        idempotency_key: waiting.key,
        id: resource.id(),
        kind,
        slug: resource.slug().as_str().to_owned(),
        name: resource.name().as_str().to_owned(),
        version: content,
      };
      let created = self.runtime.block_on(self.server.create_resource(create))?;
      return Ok((Some(created.resource), created.version));
    }
    let publish = VersionPublish {
      idempotency_key: waiting.key,
      based_on: waiting.based_on,
      body: content.body,
      references: content.references,
    };
    let version = self
      .runtime
      .block_on(self.server.publish_version(waiting.resource, publish))?;
    Ok((None, version))
  }

  fn upload(&self, hash: ContentHash) -> Result<(), SharingError> {
    let content = self.db.blob(&hash)?;
    self.runtime.block_on(self.server.upload_file(hash, content))?;
    Ok(())
  }

  /// Keeps `version`, the server's of `waiting`, with the body this computer published as its local body.
  fn confirm<B: ResourceBody + Clone>(
    &mut self,
    waiting: &WaitingPublish,
    resource: Option<&SharedResource>,
    version: &SharedVersion,
  ) -> Result<(), SharingError> {
    if let Some(shared) = resource {
      self.db.keep_shared_resource(&kept_resource(shared)?, shared.revision)?;
    }
    let local = self
      .db
      .version::<B>(waiting.resource, waiting.number)?
      .ok_or(StoreError::ResourceNotFound(waiting.resource))?;
    let confirmed = CachedVersion {
      number: version.number,
      published_at: Timestamp::from_unix_millis(version.published.at),
      publisher: convert::person(&version.published.by),
      server_body: version.body.clone(),
      hash: version.hash.clone(),
      references: version.references.iter().map(reference).collect(),
      local_body: local.body().clone(),
      // A version this computer published itself never waits for *Use v<N> here*.
      waits_here: false,
    };
    self.db.confirm_publish::<B>(waiting.version, &confirmed)?;
    Ok(())
  }

  /// Caches the versions of `resource`, of `kind`, the server has and this computer does not, as the stream would: a
  /// teammate's that widens what agents may do here waits for *Use v<N> here*. A teammate's skill is not kept here yet.
  fn catch_up(&mut self, resource: ResourceId, kind: ResourceKind) -> Result<(), SharingError> {
    if kind == ResourceKind::Skill {
      return Ok(());
    }
    let mut after = self.db.latest_version_number(resource)?;
    let mut versions = Vec::new();
    loop {
      let page = self.runtime.block_on(self.server.versions(resource, after))?;
      versions.extend(page.versions);
      let Some(next) = page.next else {
        break;
      };
      after = Some(next);
    }
    for version in &versions {
      let bound = bindings::bind_new(&mut self.db, &version.references)?;
      match kind {
        ResourceKind::Pipeline => cache_version::<Pipeline>(&mut self.db, version, &bound)?,
        ResourceKind::AgentProfile => cache_version::<AgentProfile>(&mut self.db, version, &bound)?,
        ResourceKind::Instruction => cache_version::<Instruction>(&mut self.db, version, &bound)?,
        ResourceKind::Tool => cache_version::<Tool>(&mut self.db, version, &bound)?,
        // Skills are left out above, and agent testing's kinds are never shared.
        ResourceKind::Skill | ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => {}
      }
    }
    Ok(())
  }
}

/// Whether `error` says the server was not reached, so the version waits.
fn unreached(error: &SharingError) -> bool {
  match error {
    SharingError::Offline | SharingError::SignedOut => true,
    SharingError::Client(error) => error.code().is_none(),
    _ => false,
  }
}

/// The server's error code as it writes it.
fn code_text(code: ErrorCode) -> String {
  let quoted = serde_json::to_string(&code).expect("an error code serializes as a string");
  quoted.trim_matches('"').to_owned()
}

/// A reference of the version a body was edited from, as core turns local ids back into it; `None` for a shared tool,
/// which is a resource, and for a role this version does not know.
fn base_reference(kept: &VersionReference) -> Option<Reference> {
  let name = kept.name.clone();
  match kept.target {
    ReferenceTarget::Connection(id) => Some(Reference::Connection {
      id,
      name,
      role: ConnectionRole::parse(kept.role.as_deref()?)?,
    }),
    ReferenceTarget::McpServer(id) => Some(Reference::McpServer { id, name }),
    ReferenceTarget::Secret(id) => Some(Reference::Secret {
      id,
      name,
      role: kept.role.as_deref()?.parse::<SecretKind>().ok()?,
    }),
    ReferenceTarget::Tool(_) => None,
  }
}
