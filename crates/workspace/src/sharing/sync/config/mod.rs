//! A shared project's configuration on this computer (`docs/architecture/server.md`, "Shared configuration"): the
//! team's resources and their published versions, read from the server's resource routes for each resource the
//! stream's snapshot names and each `resource.published` event, and kept in the cache twice — the server's body as it
//! is, and a local body with this computer's bindings in place of the references ([`bindings`]). Runs pin and read the
//! local body as they do any version. A reference that is not bound keeps the publisher's id, which matches nothing
//! here, so preflight reports it missing. When a person binds a reference, the local bodies that name it are written
//! again ([`bind`]). Skills are not kept yet: their parts travel apart from their bodies, by hash. An admin publishes
//! from this computer through [`RemoteResourceStore`].

mod bindings;
mod publish;

use std::path::Path;

use orchester_api::{
  Reference as SharedReference, ReferenceTarget as SharedTarget, SharedResource, SharedResourceKind, SharedVersion,
};
use orchester_core::{
  name::Name,
  resource::{AgentProfile, Instruction, Pipeline, Resource, ResourceBody, ResourceKind, Slug, Tool},
  sharing::{Binding, resolve},
  store::{ConnectionStore, McpServerStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{
  Bound, BoundBy, BoundReference, CachedVersion, NamingVersion, ProjectDb, PublishState, ReferenceBinding,
  ReferenceTarget, StoreError, VersionReference,
};
use serde_json::Value;

pub use self::publish::{ConfigServer, RemoteResourceStore};
use super::stream::{Source, blocking};
use crate::sharing::{SharingError, project::convert};

/// Keeps `resource` as the server has it, with every version this computer does not have yet. While a version of it
/// published here is being sent, its versions are read by the sending ([`RemoteResourceStore`]) instead: the server
/// may have numbered that version already.
pub(super) async fn sync(source: &impl Source, root: &Path, resource: SharedResource) -> Result<(), SharingError> {
  if resource.kind == SharedResourceKind::Skill {
    tracing::debug!(resource = %resource.id, "a shared skill is not kept on this computer yet");
    return Ok(());
  }
  let id = resource.id;
  let (cached, sending) = blocking(root, move |root| {
    let db = ProjectDb::open(root)?;
    let sending = db
      .waiting_publishes()?
      .iter()
      .any(|waiting| waiting.resource == id && waiting.state == PublishState::Waiting);
    Ok((db.latest_version_number(id)?, sending))
  })
  .await?;
  let mut versions = Vec::new();
  if resource.latest > cached && !sending {
    let mut after = cached;
    loop {
      let page = source.versions(id, after).await?;
      versions.extend(page.versions);
      let Some(next) = page.next else {
        break;
      };
      after = Some(next);
    }
  }
  blocking(root, move |root| cache(root, &resource, &versions)).await
}

/// Binds the reference of `choice` to the local connection, MCP server or secret a person chose (`None` unbinds it),
/// and writes again the local body of every cached version that names it; returns how many changed. Only a person
/// binds: binding an MCP server or a secret widens what agents may do or what a tool sends, so no MCP tool reaches
/// this.
pub fn bind(root: &Path, choice: BoundReference) -> Result<usize, SharingError> {
  let mut db = ProjectDb::open(root)?;
  let target = bindings::binding_target(choice);
  let kept = db
    .reference_bindings()?
    .into_iter()
    .find(|kept| bindings::binding_target(kept.reference) == target)
    .ok_or_else(|| not_found(target))?;
  check_local(&db, choice)?;
  db.set_reference_binding(&ReferenceBinding {
    reference: by_person(choice),
    changed_at: Timestamp::now(),
    ..kept
  })?;
  rewrite(&mut db, target)
}

/// [`write`], except that a resource this computer cannot keep is left out with a warning instead of stopping the
/// stream, which would read it again and fail again at every reconnect.
fn cache(root: &Path, resource: &SharedResource, versions: &[SharedVersion]) -> Result<(), SharingError> {
  match write(root, resource, versions) {
    Err(SharingError::Store(error @ (StoreError::DuplicateSlug { .. } | StoreError::ResourceKindMismatch { .. }))) => {
      tracing::warn!(%error, resource = %resource.id, "a shared resource clashes with one of this project's");
      Ok(())
    }
    Err(error @ SharingError::InvalidAnswer(_)) => {
      tracing::warn!(%error, resource = %resource.id, "a shared resource's version cannot be read");
      Ok(())
    }
    written => written,
  }
}

fn write(root: &Path, shared: &SharedResource, versions: &[SharedVersion]) -> Result<(), SharingError> {
  let mut db = ProjectDb::open(root)?;
  db.keep_shared_resource(&resource(shared)?, shared.revision)?;
  for version in versions {
    let bindings = bindings::bind_new(&mut db, &version.references)?;
    match shared.kind {
      SharedResourceKind::Pipeline => cache_version::<Pipeline>(&mut db, version, &bindings)?,
      SharedResourceKind::AgentProfile => cache_version::<AgentProfile>(&mut db, version, &bindings)?,
      SharedResourceKind::Instruction => cache_version::<Instruction>(&mut db, version, &bindings)?,
      SharedResourceKind::Tool => cache_version::<Tool>(&mut db, version, &bindings)?,
      // `sync` leaves skills out.
      SharedResourceKind::Skill => {}
    }
  }
  Ok(())
}

fn resource(shared: &SharedResource) -> Result<Resource, SharingError> {
  Ok(Resource::new(
    shared.id,
    shared.kind.core(),
    Slug::new(&shared.slug).map_err(|_| SharingError::InvalidAnswer("resource slug"))?,
    Name::new(&shared.name).map_err(|_| SharingError::InvalidAnswer("resource name"))?,
    Timestamp::from_unix_millis(shared.created.at),
  ))
}

fn cache_version<B: ResourceBody>(
  db: &mut ProjectDb,
  version: &SharedVersion,
  bindings: &[Binding],
) -> Result<(), SharingError> {
  let cached = CachedVersion {
    number: version.number,
    published_at: Timestamp::from_unix_millis(version.published.at),
    publisher: convert::person(&version.published.by),
    server_body: version.body.clone(),
    hash: version.hash.clone(),
    references: version.references.iter().map(reference).collect(),
    local_body: local_body::<B>(&version.body, bindings)?,
  };
  db.cache_shared_version(version.resource, &cached)?;
  Ok(())
}

/// `server_body` with every reference `bindings` bind replaced by the local id.
fn local_body<B: ResourceBody>(server_body: &Value, bindings: &[Binding]) -> Result<B, SharingError> {
  serde_json::from_value(resolve(server_body.clone(), bindings))
    .map_err(|_| SharingError::InvalidAnswer("resource body"))
}

fn reference(shared: &SharedReference) -> VersionReference {
  VersionReference {
    target: match shared.target {
      SharedTarget::Connection { id } => ReferenceTarget::Connection(id),
      SharedTarget::McpServer { id } => ReferenceTarget::McpServer(id),
      SharedTarget::Secret { id } => ReferenceTarget::Secret(id),
      SharedTarget::Tool { id } => ReferenceTarget::Tool(id),
    },
    name: shared.name.clone(),
    role: shared.role.clone(),
  }
}

/// No cached version names `target`, so this computer has no binding of it to change.
fn not_found(target: ReferenceTarget) -> StoreError {
  match target {
    ReferenceTarget::Connection(id) | ReferenceTarget::McpServer(id) => StoreError::ConnectionNotFound(id),
    ReferenceTarget::Secret(id) => StoreError::SecretNotFound(id),
    ReferenceTarget::Tool(id) => StoreError::ResourceNotFound(id),
  }
}

/// Fails unless the local entity `choice` binds to exists here; a secret must not be archived.
fn check_local(db: &ProjectDb, choice: BoundReference) -> Result<(), StoreError> {
  let missing = match choice {
    BoundReference::Connection { bound: Some(bound), .. } => db
      .connection(bound.local)?
      .is_none()
      .then_some(StoreError::ConnectionNotFound(bound.local)),
    BoundReference::McpServer { bound: Some(bound), .. } => db
      .mcp_server(bound.local)?
      .is_none()
      .then_some(StoreError::ConnectionNotFound(bound.local)),
    BoundReference::Secret { bound: Some(bound), .. } => {
      (db.secret(bound.local)?.is_none_or(|secret| secret.is_archived()))
        .then_some(StoreError::SecretNotFound(bound.local))
    }
    BoundReference::Connection { bound: None, .. }
    | BoundReference::McpServer { bound: None, .. }
    | BoundReference::Secret { bound: None, .. } => None,
  };
  missing.map_or(Ok(()), Err)
}

/// `choice` as a person's binding, whoever it says bound it.
fn by_person(choice: BoundReference) -> BoundReference {
  match choice {
    BoundReference::Connection { reference, bound } => BoundReference::Connection {
      reference,
      bound: person(bound),
    },
    BoundReference::McpServer { reference, bound } => BoundReference::McpServer {
      reference,
      bound: person(bound),
    },
    BoundReference::Secret { reference, bound } => BoundReference::Secret {
      reference,
      bound: person(bound),
    },
  }
}

fn person<I>(bound: Option<Bound<I>>) -> Option<Bound<I>> {
  bound.map(|bound| Bound {
    local: bound.local,
    by: BoundBy::Person,
  })
}

/// Writes again the local body of every cached version that names `target`, with the bindings as they are now.
fn rewrite(db: &mut ProjectDb, target: ReferenceTarget) -> Result<usize, SharingError> {
  let bindings = bindings::bound(&db.reference_bindings()?);
  let mut rewritten = 0;
  for naming in db.versions_naming(target)? {
    let changed = match naming.kind {
      ResourceKind::Pipeline => rewrite_version::<Pipeline>(db, naming, &bindings)?,
      ResourceKind::AgentProfile => rewrite_version::<AgentProfile>(db, naming, &bindings)?,
      ResourceKind::Instruction => rewrite_version::<Instruction>(db, naming, &bindings)?,
      ResourceKind::Tool => rewrite_version::<Tool>(db, naming, &bindings)?,
      // Skills are not kept here yet, and agent testing's kinds are never shared.
      ResourceKind::Skill | ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => false,
    };
    rewritten += usize::from(changed);
  }
  Ok(rewritten)
}

fn rewrite_version<B: ResourceBody>(
  db: &mut ProjectDb,
  naming: NamingVersion,
  bindings: &[Binding],
) -> Result<bool, SharingError> {
  let Some(cached) = db.cached_version::<B>(naming.resource, naming.number)? else {
    return Ok(false);
  };
  let local = local_body::<B>(&cached.server_body, bindings)?;
  if local == cached.local_body {
    return Ok(false);
  }
  Ok(db.rewrite_local_body(naming.resource, naming.number, &local)?)
}
