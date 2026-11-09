//! A shared project's configuration on this computer (`docs/architecture/server.md`, "Shared configuration"): the
//! team's resources and their published versions, read from the server's resource routes for each resource the
//! stream's snapshot names and each `resource.published` event, and kept in the cache twice — the server's body as it
//! is, and the local body runs pin and read as they do any version. A team body names no id of a computer
//! (`docs/design/team-configuration.md`), so the two are the same, except a team profile's, which runs on the
//! connection this computer picked for it ([`profile`]). A teammate's version that widens what agents may do here waits
//! for the person's *Use v<N> here* ([`waiting`]). Skills are not kept yet: their parts travel apart from their bodies,
//! by hash. An admin publishes from this computer through [`RemoteResourceStore`]; Share uploads the project's own
//! configuration ([`upload`]).

mod profile;
mod publish;
mod share_fill_ins;
mod share_mcp;
mod team_body;
mod upload;
mod waiting;

use std::path::Path;

use orchester_api::{ReferenceTarget as SharedTarget, SharedResource, SharedResourceKind, SharedVersion};
use orchester_core::{
  name::Name,
  resource::{Instruction, Pipeline, Resource, ResourceBody, Slug, Tool},
  sharing::SharedBody,
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb, PublishState, ReferenceTarget, StoreError};
use serde_json::Value;

use self::profile::cache_profile;
pub(crate) use self::{
  profile::{refresh as refresh_profile, refresh_all as refresh_team_profiles},
  team_body::{non_team_name, team_executors, team_form},
};
pub use self::{
  publish::{ConfigServer, RemoteResourceStore},
  share_fill_ins::{FillInOutcome, ShareFillIns},
  upload::{keep_uploaded_configuration, upload_configuration},
  waiting::{Waiting, use_here, waiting},
};
use super::stream::{Source, blocking};
use crate::sharing::{SharingError, project::convert};

/// Keeps `resource` as the server has it, with every version this computer does not have yet. While a version of it
/// published here is being sent, its versions are read by the sending ([`RemoteResourceStore`]) instead: the server
/// may have numbered that version already. One archived only now stops the runs that pin it ([`Source::archived`]).
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
  tracing::debug!(
    resource = %id,
    slug = %resource.slug,
    kind = ?resource.kind,
    latest = ?resource.latest,
    ?cached,
    sending,
    read = versions.len(),
    "a shared resource is kept"
  );
  if blocking(root, move |root| cache(root, &resource, &versions)).await? {
    source.archived(id, None).await;
  }
  Ok(())
}

/// [`write`], except that a resource this computer cannot keep is left out with a warning instead of stopping the
/// stream, which would read it again and fail again at every reconnect.
fn cache(root: &Path, resource: &SharedResource, versions: &[SharedVersion]) -> Result<bool, SharingError> {
  match write(root, resource, versions) {
    Err(SharingError::Store(error @ (StoreError::DuplicateSlug { .. } | StoreError::ResourceKindMismatch { .. }))) => {
      tracing::warn!(%error, resource = %resource.id, "a shared resource clashes with one of this project's");
      Ok(false)
    }
    Err(error @ SharingError::InvalidAnswer(_)) => {
      tracing::warn!(%error, resource = %resource.id, "a shared resource's version cannot be read");
      Ok(false)
    }
    written => written,
  }
}

/// Keeps `shared` with `versions`, archived or not as the server has it; whether it was archived only now.
fn write(root: &Path, shared: &SharedResource, versions: &[SharedVersion]) -> Result<bool, SharingError> {
  let mut db = ProjectDb::open(root)?;
  db.keep_shared_resource(&resource(shared)?, shared.revision)?;
  let archived_now = db.mark_shared_archived(shared.id, shared.archived, Timestamp::now())? && shared.archived;
  for version in versions {
    match shared.kind {
      SharedResourceKind::Pipeline => cache_version::<Pipeline>(&mut db, version)?,
      SharedResourceKind::AgentProfile => cache_profile(&mut db, version)?,
      SharedResourceKind::Instruction => cache_version::<Instruction>(&mut db, version)?,
      SharedResourceKind::Tool => cache_version::<Tool>(&mut db, version)?,
      SharedResourceKind::TeamReviewer => cache_version::<orchester_core::resource::TeamReviewer>(&mut db, version)?,
      SharedResourceKind::McpServer => cache_version::<orchester_core::resource::TeamMcpServer>(&mut db, version)?,
      // `sync` leaves skills out.
      SharedResourceKind::Skill => {}
    }
  }
  Ok(archived_now)
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

fn cache_version<B: ResourceBody + SharedBody>(
  db: &mut ProjectDb,
  version: &SharedVersion,
) -> Result<(), SharingError> {
  cache_body(db, version, body::<B>(version.body.clone())?)
}

/// Keeps `version` as the server has it, with `local_body` as the body this computer runs.
fn cache_body<B: ResourceBody + SharedBody>(
  db: &mut ProjectDb,
  version: &SharedVersion,
  local_body: B,
) -> Result<(), SharingError> {
  let waits_here = waiting::waits(db, version.resource, version.number, &local_body)?;
  let cached = CachedVersion {
    number: version.number,
    published_at: Timestamp::from_unix_millis(version.published.at),
    publisher: convert::person(&version.published.by),
    server_body: version.body.clone(),
    hash: version.hash.clone(),
    references: version.references.iter().map(reference).collect(),
    local_body,
    waits_here,
  };
  db.cache_shared_version(version.resource, &cached)?;
  Ok(())
}

fn body<B: ResourceBody>(value: Value) -> Result<B, SharingError> {
  serde_json::from_value(value).map_err(|_| SharingError::InvalidAnswer("resource body"))
}

/// What a reference the server sent is, as the cache keeps it.
fn reference(shared: &SharedTarget) -> ReferenceTarget {
  match shared {
    SharedTarget::CliKind { cli } => ReferenceTarget::CliKind(*cli),
    SharedTarget::TeamResource { kind, id } => ReferenceTarget::TeamResource {
      kind: kind.core(),
      id: *id,
    },
    SharedTarget::SecretName { name } => ReferenceTarget::SecretName(name.clone()),
  }
}
