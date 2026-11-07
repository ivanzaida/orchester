//! A shared project's configuration on this computer (`docs/architecture/server.md`, "Shared configuration"): the
//! team's resources and their published versions, read from the server's resource routes for each resource the
//! stream's snapshot names and each `resource.published` event, and kept in the cache twice — the server's body as it
//! is, and the local body runs pin and read as they do any version. A team body names no id of a computer
//! (`docs/design/team-configuration.md`), so the two are the same; a version cached before that model still holds the
//! publisher's ids, and when a person binds one of them the local bodies that name it are written again ([`bind`]).
//! A teammate's version that widens what agents may do here waits for the person's *Use v<N> here*
//! ([`waiting`]). Skills are not kept yet: their parts travel apart from their bodies, by hash. An admin publishes
//! from this computer through [`RemoteResourceStore`]; Share uploads the project's own configuration ([`upload`]).

mod bindings;
mod bindings_here;
mod profile;
mod publish;
mod team_body;
mod upload;
mod waiting;

use std::path::Path;

use orchester_api::{ReferenceTarget as SharedTarget, SharedResource, SharedResourceKind, SharedVersion};
use orchester_core::{
  name::Name,
  resource::{AgentProfile, Instruction, Pipeline, Resource, ResourceBody, ResourceKind, Slug, Tool},
  run::Answerer,
  sharing::{Binding, SharedBody, resolve},
  store::{ConnectionStore, McpServerStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{
  Bound, BoundBy, BoundReference, CachedVersion, NamingVersion, ProjectDb, PublishState, ReferenceBinding,
  ReferenceTarget, StoreError, VersionReference,
};
use serde_json::Value;

use self::profile::cache_profile;
pub use self::{
  bindings_here::{BindingsHere, HereReference, Needer, Standing, chosen_at},
  publish::{ConfigServer, RemoteResourceStore},
  upload::{keep_uploaded_configuration, upload_configuration},
  waiting::{Waiting, use_here, waiting},
};
pub(crate) use self::{
  profile::{is_local_form, refresh as refresh_profile, refresh_all as refresh_team_profiles},
  team_body::{non_team_name, team_executors, team_form},
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
    source.archived(id).await;
  }
  Ok(())
}

/// Binds the reference of `choice` to the local connection, MCP server or secret a person chose (`None` unbinds it),
/// and writes again the local body of every cached version that names it; returns how many changed. Only a person
/// binds: binding an MCP server or a secret widens what agents may do or what a tool sends, so automation is refused
/// ([`SharingError::PersonOnly`]) and no MCP tool reaches this. Only versions cached before the team configuration
/// model have references to bind; ORC-845 removes this with the bindings.
pub fn bind(root: &Path, choice: BoundReference, by: Answerer) -> Result<usize, SharingError> {
  waiting::person_only(by)?;
  let mut db = ProjectDb::open(root)?;
  let target = bindings::binding_target(choice);
  let kept = db
    .reference_bindings()?
    .into_iter()
    .find(|kept| bindings::binding_target(kept.reference) == target)
    .ok_or_else(|| not_found(choice))?;
  check_local(&db, choice, kept.role.as_deref())?;
  db.set_reference_binding(&ReferenceBinding {
    reference: by_person(choice),
    changed_at: Timestamp::now(),
    ..kept
  })?;
  rewrite(&mut db, &target)
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

/// `server_body` of a version cached before the team configuration model, with every reference `bindings` bind
/// replaced by the local id. ORC-845 removes this with the bindings: a team body then runs as the server sent it.
fn local_body<B: ResourceBody>(server_body: &Value, bindings: &[Binding]) -> Result<B, SharingError> {
  body(resolve(server_body.clone(), bindings))
}

/// What a reference the server sent is, as the cache keeps it; the team's references need no name or role.
fn reference(shared: &SharedTarget) -> VersionReference {
  let target = match shared {
    SharedTarget::CliKind { cli } => ReferenceTarget::CliKind(*cli),
    SharedTarget::TeamResource { kind, id } => ReferenceTarget::TeamResource {
      kind: kind.core(),
      id: *id,
    },
    SharedTarget::SecretName { name } => ReferenceTarget::SecretName(name.clone()),
  };
  VersionReference {
    target,
    name: String::new(),
    role: None,
  }
}

/// No cached version names the reference `choice` binds, so this computer has no binding of it to change.
fn not_found(choice: BoundReference) -> StoreError {
  match choice {
    BoundReference::Connection { reference, .. } | BoundReference::McpServer { reference, .. } => {
      StoreError::ConnectionNotFound(reference)
    }
    BoundReference::Secret { reference, .. } => StoreError::SecretNotFound(reference),
  }
}

/// Fails unless the local entity exists here, a connection matches its role, and a secret is not archived.
fn check_local(db: &ProjectDb, choice: BoundReference, role: Option<&str>) -> Result<(), SharingError> {
  let missing = match choice {
    BoundReference::Connection { bound: Some(bound), .. } => {
      let connection = db
        .connection(bound.local)?
        .ok_or(StoreError::ConnectionNotFound(bound.local))?;
      let actual = orchester_core::sharing::ConnectionRole::of(connection.kind());
      if Some(actual.as_str()) != role {
        return Err(SharingError::ConnectionKindMismatch {
          expected: role.map(str::to_owned),
          actual,
        });
      }
      None
    }
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
  missing.map_or(Ok(()), |error| Err(error.into()))
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
fn rewrite(db: &mut ProjectDb, target: &ReferenceTarget) -> Result<usize, SharingError> {
  let bindings = bindings::bound(&db.reference_bindings()?, &db.connections()?);
  let mut rewritten = 0;
  for naming in db.versions_naming(target)? {
    let changed = match naming.kind {
      ResourceKind::Pipeline => rewrite_version::<Pipeline>(db, naming, &bindings)?,
      ResourceKind::AgentProfile => rewrite_version::<AgentProfile>(db, naming, &bindings)?,
      ResourceKind::Instruction => rewrite_version::<Instruction>(db, naming, &bindings)?,
      ResourceKind::Tool => rewrite_version::<Tool>(db, naming, &bindings)?,
      ResourceKind::TeamReviewer => rewrite_version::<orchester_core::resource::TeamReviewer>(db, naming, &bindings)?,
      ResourceKind::McpServer => rewrite_version::<orchester_core::resource::TeamMcpServer>(db, naming, &bindings)?,
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
