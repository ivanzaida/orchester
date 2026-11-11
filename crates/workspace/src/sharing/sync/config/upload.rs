//! Share's upload of the project's configuration (`docs/architecture/server.md`, "Migration", S5): every pipeline,
//! agent profile, instruction, skill and tool goes to the server through `POST …/resources/import` with its id, slug
//! and name and every published version, numbers and times kept, after the skill parts its versions list, by hash. A
//! resource with no published version stays here as its draft, and drafts stay local, based on the same versions.
//! Then each uploaded resource is kept as the server's with its versions as the server numbered them; their local
//! bodies stay the bodies this computer published. An agent profile is uploaded in the team's form, with the references
//! its body names ([`team_body::profile`]), and the local MCP servers the profiles choose as team MCP servers
//! ([`McpPlan`]); any other body that names a connection, MCP server or secret of this computer is not uploaded
//! ([`team_body::unchanged`]). Every resource that cannot go is collected, and the upload fails with
//! [`SharingError::ShareRefused`] listing them all, with nothing sent.

use std::{collections::BTreeSet, path::Path};

use orchester_api::{
  ResourcesImport, SharedResource, SharedResourceKind, SharedVersion, UploadResource, UploadVersion,
};
use orchester_core::{
  blob::ContentHash,
  id::{IdempotencyKey, ResourceId},
  resource::{
    AgentProfile, Instruction, Pipeline, Resource, ResourceBody, Skill, TeamExecutors, TeamMcpServer, TeamReviewer,
    Tool, Version,
  },
  sharing::LocalEntities,
  store::{BlobStore, ConnectionStore, McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb, StoreError};

use super::{ConfigServer, blocking, body, reference, resource, share_mcp::McpPlan, team_body};
use crate::sharing::{ShareRefusal, ShareRefusalReason, SharingError, project::convert, team_edit::Publication};

/// The kinds Share uploads from the project's resources, those other resources name first: a tool before the profiles
/// that use it, profiles before the pipelines whose steps run them. The team MCP servers go before all of them
/// ([`McpPlan`]): they are made from the local MCP servers, which are not resources.
const KINDS: [SharedResourceKind; 6] = [
  SharedResourceKind::Tool,
  SharedResourceKind::Skill,
  SharedResourceKind::Instruction,
  SharedResourceKind::AgentProfile,
  SharedResourceKind::Pipeline,
  SharedResourceKind::TeamReviewer,
];

/// What one import request carries at most, under the server's 1 MiB request limit; a resource whose history alone is
/// larger goes in a request of its own.
const BATCH_BYTES: usize = 1_000_000;

/// Uploads the configuration of the project at `root` to `server`: the skill parts first, then the resources in
/// batches; returns the resources as the server keeps them.
pub async fn upload_configuration(
  server: &impl ConfigServer,
  root: &Path,
) -> Result<Vec<SharedResource>, SharingError> {
  let (resources, parts) = blocking(root, |root| read(&ProjectDb::open(root)?)).await?;
  for hash in parts {
    let read = hash.clone();
    let content = blocking(root, move |root| Ok(ProjectDb::open(root)?.blob(&read)?)).await?;
    server.upload_file(hash, content).await?;
  }
  let mut uploaded = Vec::new();
  for batch in batches(resources) {
    // The batch's first resource names it: a retry of the Share makes the same batches, which the server answers once.
    let key = IdempotencyKey::from_uuid(batch[0].id.as_uuid());
    let import = ResourcesImport {
      idempotency_key: key,
      resources: batch,
    };
    uploaded.extend(server.import_resources(import).await?);
  }
  Ok(uploaded)
}

/// Keeps each of `uploaded` in the project at `root` as the server's, with its versions as `server` numbered them.
pub async fn keep_uploaded_configuration(
  server: &impl ConfigServer,
  root: &Path,
  uploaded: Vec<SharedResource>,
) -> Result<(), SharingError> {
  for shared in uploaded {
    let mut versions = Vec::new();
    let mut after = None;
    loop {
      let page = server.versions(shared.id, after).await?;
      versions.extend(page.versions);
      let Some(next) = page.next else {
        break;
      };
      after = Some(next);
    }
    blocking(root, move |root| keep(&mut ProjectDb::open(root)?, &shared, &versions)).await?;
  }
  Ok(())
}

/// What Share has gathered of the project so far.
#[derive(Default)]
struct Collected {
  uploads: Vec<UploadResource>,
  parts: BTreeSet<ContentHash>,
  refusals: Vec<ShareRefusal>,
  /// The profiles and tools to upload, which pipelines may run; a refused one counts, as the Share is refused whole.
  profiles: Vec<ResourceId>,
  tools: Vec<ResourceId>,
}

/// Every resource of `db` Share uploads, with the skill parts their versions list. Fails with
/// [`SharingError::ShareRefused`] listing every resource that cannot go in the team's form, before anything is sent.
fn read(db: &ProjectDb) -> Result<(Vec<UploadResource>, BTreeSet<ContentHash>), SharingError> {
  let (connections, mcp_servers, secrets) = (db.connections()?, db.mcp_servers()?, db.secrets()?);
  let entities = LocalEntities {
    connections: &connections,
    mcp_servers: &mcp_servers,
    secrets: &secrets,
  };
  let mcp = McpPlan::read(db)?;
  let mut found = Collected {
    uploads: mcp.uploads()?,
    refusals: mcp.refusals.clone(),
    ..Collected::default()
  };
  for kind in KINDS {
    for kept in db.resources(kind.core())? {
      collect(db, (entities, &mcp), kind, &kept, &mut found)?;
    }
  }
  if !found.refusals.is_empty() {
    return Err(SharingError::ShareRefused {
      refusals: found.refusals,
    });
  }
  Ok((found.uploads, found.parts))
}

/// Adds `kept` to `found`: its upload, or the refusal that it cannot be one.
fn collect(
  db: &ProjectDb,
  (entities, mcp): (LocalEntities<'_>, &McpPlan),
  kind: SharedResourceKind,
  kept: &Resource,
  found: &mut Collected,
) -> Result<(), SharingError> {
  let versions = match versions_of(db, (entities, mcp), kind, kept, found) {
    Ok(versions) => versions,
    Err(error) => {
      found.refusals.push(ShareRefusal {
        resource: kept.id(),
        name: kept.name().as_str().to_owned(),
        reason: ShareRefusalReason::of(error)?,
      });
      note_executor(found, kind, kept.id());
      return Ok(());
    }
  };
  if versions.is_empty() {
    return Ok(());
  }
  note_executor(found, kind, kept.id());
  found.uploads.push(UploadResource {
    id: kept.id(),
    kind,
    slug: kept.slug().as_str().to_owned(),
    name: kept.name().as_str().to_owned(),
    archived: false,
    versions,
  });
  Ok(())
}

fn note_executor(found: &mut Collected, kind: SharedResourceKind, resource: ResourceId) {
  match kind {
    SharedResourceKind::AgentProfile => found.profiles.push(resource),
    SharedResourceKind::Tool => found.tools.push(resource),
    _ => {}
  }
}

/// The versions of `kept` to upload, each in the team's form; the skill parts they list join `found`'s.
fn versions_of(
  db: &ProjectDb,
  (entities, mcp): (LocalEntities<'_>, &McpPlan),
  kind: SharedResourceKind,
  kept: &Resource,
  found: &mut Collected,
) -> Result<Vec<UploadVersion>, SharingError> {
  match kind {
    SharedResourceKind::Pipeline => {
      let team = TeamExecutors::new(found.profiles.iter().copied(), found.tools.iter().copied());
      check_latest_pipeline(db, kept, &team)?;
      versions::<Pipeline>(db, kept, entities)
    }
    SharedResourceKind::AgentProfile => profile_versions(db, kept, mcp),
    SharedResourceKind::Instruction => versions::<Instruction>(db, kept, entities),
    SharedResourceKind::Tool => tool_versions(db, kept),
    SharedResourceKind::TeamReviewer => reviewer_versions(db, kept),
    SharedResourceKind::McpServer => versions::<TeamMcpServer>(db, kept, entities),
    SharedResourceKind::Skill => {
      for version in db.versions::<Skill>(kept.id())? {
        found
          .parts
          .extend(version.body().files().iter().map(|file| file.content.clone()));
      }
      versions::<Skill>(db, kept, entities)
    }
  }
}

/// Fails when the newest version of the pipeline `resource` names a profile or tool Share does not upload: the team
/// would hold a pipeline naming ids it does not have. Older versions are history; a profile or tool removed since may
/// still be named there.
fn check_latest_pipeline(db: &ProjectDb, resource: &Resource, team: &TeamExecutors) -> Result<(), SharingError> {
  match db.latest_version::<Pipeline>(resource.id())? {
    Some(latest) => team_body::check_pipeline(db, resource.id(), latest.body(), team),
    None => Ok(()),
  }
}

/// `resource`'s published versions, oldest first, which name nothing of `entities`.
fn versions<B: ResourceBody>(
  db: &ProjectDb,
  resource: &Resource,
  entities: LocalEntities<'_>,
) -> Result<Vec<UploadVersion>, SharingError> {
  db.versions::<B>(resource.id())?
    .into_iter()
    .map(|version| {
      let body = serde_json::to_value(version.body()).map_err(|error| StoreError::Corrupt {
        what: "resource body",
        detail: error.to_string(),
      })?;
      let publication = team_body::unchanged(resource.id(), body, entities)?;
      Ok(upload(&version, publication))
    })
    .collect()
}

/// The published versions of the agent profile `resource`, oldest first, in the team's form: its MCP servers are the
/// team MCP servers `mcp` plans.
fn profile_versions(db: &ProjectDb, resource: &Resource, mcp: &McpPlan) -> Result<Vec<UploadVersion>, SharingError> {
  let versions = db.versions::<AgentProfile>(resource.id())?;
  let team_servers = mcp.team_ids();
  versions
    .iter()
    .enumerate()
    .map(|(index, version)| {
      let adjusted = mcp.adjusted(version.body(), index + 1 == versions.len());
      let publication = team_body::profile_naming(db, resource.id(), &adjusted, &team_servers)?;
      Ok(upload(version, publication))
    })
    .collect()
}

/// The published versions of the tool `resource`, oldest first, each in the team's form: its secret goes by the name it
/// has in the project, archived or not ([`team_body::named_secrets`]), and an id of this computer never goes. A version
/// whose secret the project has no row for is refused ([`SharingError::ToolSecretGone`]).
fn tool_versions(db: &ProjectDb, resource: &Resource) -> Result<Vec<UploadVersion>, SharingError> {
  let secrets = team_body::named_secrets(db)?;
  db.versions::<Tool>(resource.id())?
    .iter()
    .map(|version| {
      Ok(upload(
        version,
        team_body::tool(resource.id(), version.body(), &secrets)?,
      ))
    })
    .collect()
}

/// The published versions of the team reviewer `resource`, oldest first. A reviewer's body is already the team's.
fn reviewer_versions(db: &ProjectDb, resource: &Resource) -> Result<Vec<UploadVersion>, SharingError> {
  db.versions::<TeamReviewer>(resource.id())?
    .iter()
    .map(|version| Ok(upload(version, team_body::reviewer(version.body())?)))
    .collect()
}

fn upload<B>(version: &Version<B>, publication: Publication) -> UploadVersion {
  UploadVersion {
    number: version.number(),
    references: publication.references,
    body: publication.body,
    published_at: version.published_at().unix_millis(),
  }
}

/// `resources` in import requests of at most [`BATCH_BYTES`] each, in their order.
fn batches(resources: Vec<UploadResource>) -> Vec<Vec<UploadResource>> {
  let mut batches: Vec<Vec<UploadResource>> = Vec::new();
  let mut size = 0;
  for upload in resources {
    let bytes = serde_json::to_vec(&upload)
      .expect("an upload of ids, text and JSON values serializes")
      .len();
    match batches.last_mut() {
      Some(batch) if size + bytes <= BATCH_BYTES => {
        batch.push(upload);
        size += bytes;
      }
      _ => {
        batches.push(vec![upload]);
        size = bytes;
      }
    }
  }
  batches
}

fn keep(db: &mut ProjectDb, shared: &SharedResource, versions: &[SharedVersion]) -> Result<(), SharingError> {
  db.keep_shared_resource(&resource(shared)?, shared.revision)?;
  for version in versions {
    match shared.kind {
      SharedResourceKind::Pipeline => keep_version::<Pipeline>(db, version)?,
      SharedResourceKind::AgentProfile => keep_version::<AgentProfile>(db, version)?,
      SharedResourceKind::Instruction => keep_version::<Instruction>(db, version)?,
      SharedResourceKind::Skill => keep_version::<Skill>(db, version)?,
      SharedResourceKind::Tool => keep_version::<Tool>(db, version)?,
      SharedResourceKind::TeamReviewer => keep_version::<TeamReviewer>(db, version)?,
      SharedResourceKind::McpServer => keep_team_mcp_version(db, version)?,
    }
  }
  Ok(())
}

/// Keeps `version` as the server confirmed it, with the body this computer published as its local body.
fn keep_version<B: ResourceBody + Clone>(db: &mut ProjectDb, version: &SharedVersion) -> Result<(), SharingError> {
  let local = db
    .version::<B>(version.resource, version.number)?
    .ok_or(StoreError::ResourceNotFound(version.resource))?;
  db.cache_shared_version(version.resource, &confirmed(version, local.body().clone()))?;
  Ok(())
}

/// Keeps the team MCP server `version` as the server confirmed it. The project has no local copy of it (it was made
/// from a local MCP server, which stays as it is), so the server's body is the local one too.
fn keep_team_mcp_version(db: &mut ProjectDb, version: &SharedVersion) -> Result<(), SharingError> {
  let local = body::<TeamMcpServer>(version.body.clone())?;
  db.cache_shared_version(version.resource, &confirmed(version, local))?;
  Ok(())
}

fn confirmed<B>(version: &SharedVersion, local_body: B) -> CachedVersion<B> {
  CachedVersion {
    number: version.number,
    published_at: Timestamp::from_unix_millis(version.published.at),
    publisher: convert::person(&version.published.by),
    server_body: version.body.clone(),
    hash: version.hash.clone(),
    references: version.references.iter().map(reference).collect(),
    local_body,
  }
}
