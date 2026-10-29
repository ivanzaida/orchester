//! Share's upload of the project's configuration (`docs/architecture/server.md`, "Migration", S5): every pipeline,
//! agent profile, instruction, skill and tool goes to the server through `POST …/resources/import` with its id, slug
//! and name and every published version, numbers and times kept, after the skill parts its versions list, by hash. A
//! resource with no published version stays here as its draft, and drafts stay local, based on the same versions.
//! Then each uploaded resource is kept as the server's with its versions as the server numbered them; their local
//! bodies stay the bodies this computer published, and the references they make of its own connections, MCP servers
//! and secrets are bound to those same entities, so nothing changes for its runs.

use std::{collections::BTreeSet, path::Path};

use orchester_api::{
  ResourcesImport, SharedResource, SharedResourceKind, SharedVersion, UploadResource, UploadVersion,
};
use orchester_core::{
  blob::ContentHash,
  id::IdempotencyKey,
  resource::{AgentProfile, Instruction, Pipeline, Resource, ResourceBody, Skill, Tool},
  sharing::{LocalEntities, named},
  store::{BlobStore, ConnectionStore, McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb, StoreError};

use super::{ConfigServer, bindings, blocking, reference, resource, shared_reference};
use crate::sharing::{SharingError, project::convert};

/// The kinds Share uploads, those other resources name first: a tool before the profiles that use it, profiles before
/// the pipelines whose steps run them.
const KINDS: [SharedResourceKind; 5] = [
  SharedResourceKind::Tool,
  SharedResourceKind::Skill,
  SharedResourceKind::Instruction,
  SharedResourceKind::AgentProfile,
  SharedResourceKind::Pipeline,
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

/// Every resource of `db` Share uploads, with the skill parts their versions list.
fn read(db: &ProjectDb) -> Result<(Vec<UploadResource>, BTreeSet<ContentHash>), SharingError> {
  let (connections, mcp_servers, secrets) = (db.connections()?, db.mcp_servers()?, db.secrets()?);
  let entities = LocalEntities {
    connections: &connections,
    mcp_servers: &mcp_servers,
    secrets: &secrets,
  };
  let mut uploads = Vec::new();
  let mut parts = BTreeSet::new();
  for kind in KINDS {
    for kept in db.resources(kind.core())? {
      let versions = match kind {
        SharedResourceKind::Pipeline => versions::<Pipeline>(db, &kept, entities)?,
        SharedResourceKind::AgentProfile => versions::<AgentProfile>(db, &kept, entities)?,
        SharedResourceKind::Instruction => versions::<Instruction>(db, &kept, entities)?,
        SharedResourceKind::Tool => versions::<Tool>(db, &kept, entities)?,
        SharedResourceKind::Skill => {
          for version in db.versions::<Skill>(kept.id())? {
            parts.extend(version.body().files().iter().map(|file| file.content.clone()));
          }
          versions::<Skill>(db, &kept, entities)?
        }
      };
      if versions.is_empty() {
        continue;
      }
      uploads.push(UploadResource {
        id: kept.id(),
        kind,
        slug: kept.slug().as_str().to_owned(),
        name: kept.name().as_str().to_owned(),
        archived: false,
        versions,
      });
    }
  }
  Ok((uploads, parts))
}

/// `resource`'s published versions, oldest first, each with a reference for every connection, MCP server and secret of
/// `entities` its body names.
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
      Ok(UploadVersion {
        number: version.number(),
        references: named(&body, entities).iter().map(shared_reference).collect(),
        body,
        published_at: version.published_at().unix_millis(),
      })
    })
    .collect()
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
    bindings::bind_own(db, &version.references)?;
    match shared.kind {
      SharedResourceKind::Pipeline => keep_version::<Pipeline>(db, version)?,
      SharedResourceKind::AgentProfile => keep_version::<AgentProfile>(db, version)?,
      SharedResourceKind::Instruction => keep_version::<Instruction>(db, version)?,
      SharedResourceKind::Skill => keep_version::<Skill>(db, version)?,
      SharedResourceKind::Tool => keep_version::<Tool>(db, version)?,
    }
  }
  Ok(())
}

/// Keeps `version` as the server confirmed it, with the body this computer published as its local body.
fn keep_version<B: ResourceBody + Clone>(db: &mut ProjectDb, version: &SharedVersion) -> Result<(), SharingError> {
  let local = db
    .version::<B>(version.resource, version.number)?
    .ok_or(StoreError::ResourceNotFound(version.resource))?;
  let confirmed = CachedVersion {
    number: version.number,
    published_at: Timestamp::from_unix_millis(version.published.at),
    publisher: convert::person(&version.published.by),
    server_body: version.body.clone(),
    hash: version.hash.clone(),
    references: version.references.iter().map(reference).collect(),
    local_body: local.body().clone(),
    // This computer published it, so it never waits for a *Use v<N> here*.
    waits_here: false,
  };
  db.cache_shared_version(version.resource, &confirmed)?;
  Ok(())
}
