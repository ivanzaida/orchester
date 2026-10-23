//! Import resources: the copies of another project's resources, written in one transaction, where
//! each came from (`resource_imports`), and what the import used of this project's or left
//! unresolved (`import_resolutions`).

use std::num::NonZeroU32;

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId, SecretId},
  import::{Content, CopiedResource, Entity, ImportBatch, ImportRecord, Provenance, Resolved},
  name::Name,
  resource::{Resource, VersionNumber},
  store::ImportStore,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{
  ProjectDb,
  resources::{encode, kind_text, load_resources},
};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ImportStore for ProjectDb {
  type Error = StoreError;

  fn import(&mut self, batch: &ImportBatch) -> Result<Vec<Resource>, StoreError> {
    let bodies = batch
      .copies()
      .iter()
      .map(|copy| encode_content(copy.content()))
      .collect::<Result<Vec<_>, _>>()?;
    let transaction = self.connection.transaction()?;
    let earlier = imported(&transaction, batch.key())?;
    if !earlier.is_empty() {
      return Ok(earlier);
    }
    for entity in batch.relies_on() {
      check_exists(&transaction, *entity)?;
    }
    for (copy, body) in batch.copies().iter().zip(&bodies) {
      insert(&transaction, batch.key(), copy, body)?;
    }
    for (position, resolved) in batch.resolved().iter().enumerate() {
      insert_resolved(&transaction, batch.key(), position, resolved)?;
    }
    transaction.commit()?;
    Ok(batch.copies().iter().map(|copy| copy.resource().clone()).collect())
  }

  fn import_record(&self, resource: ResourceId) -> Result<Option<ImportRecord>, StoreError> {
    let found: Option<(String, i64)> = self
      .connection
      .query_row(
        "SELECT import_key, imported_at FROM resource_imports WHERE resource_id = ?1",
        [resource.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    let Some((key, imported_at)) = found else {
      return Ok(None);
    };
    Ok(Some(ImportRecord {
      imported_at: Timestamp::from_unix_millis(imported_at),
      copies: copies(&self.connection, &key)?,
      resolved: resolutions(&self.connection, &key)?,
    }))
  }
}

/// The resources an import with `key` created, in the order it created them.
fn imported(transaction: &Transaction<'_>, key: IdempotencyKey) -> Result<Vec<Resource>, StoreError> {
  load_resources(
    transaction,
    "WHERE id IN (SELECT resource_id FROM resource_imports WHERE import_key = ?1) ORDER BY rowid",
    [key.to_string()],
  )
}

/// Fails unless the project has `entity`; a secret must not be archived.
fn check_exists(transaction: &Transaction<'_>, entity: Entity) -> Result<(), StoreError> {
  let (query, id, missing) = match entity {
    Entity::Resource(id) => (
      "SELECT EXISTS (SELECT 1 FROM resources WHERE id = ?1)",
      id.to_string(),
      StoreError::ResourceNotFound(id),
    ),
    Entity::Connection(id) => (
      "SELECT EXISTS (SELECT 1 FROM connections WHERE id = ?1)",
      id.to_string(),
      StoreError::ConnectionNotFound(id),
    ),
    Entity::McpServer(id) => (
      "SELECT EXISTS (SELECT 1 FROM mcp_servers WHERE id = ?1)",
      id.to_string(),
      StoreError::ConnectionNotFound(id),
    ),
    Entity::Secret(id) => (
      "SELECT EXISTS (SELECT 1 FROM secrets WHERE id = ?1 AND archived_at IS NULL)",
      id.to_string(),
      StoreError::SecretNotFound(id),
    ),
  };
  let exists: bool = transaction.query_row(query, [id], |row| row.get(0))?;
  if exists { Ok(()) } else { Err(missing) }
}

/// Stores `copy` as a resource whose only content is its first draft, with its provenance.
fn insert(
  transaction: &Transaction<'_>,
  key: IdempotencyKey,
  copy: &CopiedResource,
  body: &str,
) -> Result<(), StoreError> {
  let resource = copy.resource();
  let kind = kind_text(resource.kind());
  let taken: bool = transaction.query_row(
    "SELECT EXISTS (SELECT 1 FROM resources WHERE kind = ?1 AND slug = ?2)",
    params![kind, resource.slug().as_str()],
    |row| row.get(0),
  )?;
  if taken {
    return Err(StoreError::DuplicateSlug {
      kind: resource.kind(),
      slug: resource.slug().as_str().to_owned(),
    });
  }
  let (id, created_at) = (resource.id().to_string(), resource.created_at().unix_millis());
  transaction.execute(
    "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![id, kind, resource.slug().as_str(), resource.name().as_str(), created_at],
  )?;
  transaction.execute(
    "INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) VALUES (?1, ?2, NULL, ?3)",
    params![id, body, created_at],
  )?;
  let provenance = copy.provenance();
  transaction.execute(
    "INSERT INTO resource_imports (resource_id, import_key, source_project_id, source_project_name, \
     source_resource_id, source_resource_name, source_version, imported_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    params![
      id,
      key.to_string(),
      provenance.project.to_string(),
      provenance.project_name.as_str(),
      provenance.resource.to_string(),
      provenance.resource_name.as_str(),
      provenance.version.map(VersionNumber::get),
      created_at
    ],
  )?;
  Ok(())
}

/// Records what the import with `key` did with a source item it did not copy, at `position` in plan order.
fn insert_resolved(
  transaction: &Transaction<'_>,
  key: IdempotencyKey,
  position: usize,
  resolved: &Resolved,
) -> Result<(), StoreError> {
  let (kind, source) = entity_columns(resolved.source);
  let position = i64::try_from(position).map_err(|_| StoreError::Unrepresentable("import resolution position"))?;
  transaction.execute(
    "INSERT INTO import_resolutions (import_key, position, source_kind, source_id, source_name, target_id) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    params![
      key.to_string(),
      position,
      kind,
      source,
      resolved.source_name,
      resolved.target.map(|target| entity_columns(target).1)
    ],
  )?;
  Ok(())
}

/// The copies the import with `key` created, in the order it created them, with where each came from.
fn copies(connection: &Connection, key: &str) -> Result<Vec<(ResourceId, Provenance)>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT imports.resource_id, imports.source_project_id, imports.source_project_name, \
       imports.source_resource_id, COALESCE(imports.source_resource_name, resources.name), imports.source_version \
     FROM resource_imports AS imports JOIN resources ON resources.id = imports.resource_id \
     WHERE imports.import_key = ?1 ORDER BY resources.rowid",
  )?;
  let rows = statement.query_map([key], |row| {
    Ok((
      row.get::<_, String>(0)?,
      row.get::<_, String>(1)?,
      row.get::<_, String>(2)?,
      row.get::<_, String>(3)?,
      row.get::<_, String>(4)?,
      row.get::<_, Option<u32>>(5)?,
    ))
  })?;
  rows
    .map(|row| {
      let (id, project, project_name, source, source_name, version) = row?;
      let provenance = Provenance {
        project: parse::<ProjectId>("imported resource's source project", &project)?,
        project_name: stored_name("imported resource's source project name", &project_name)?,
        resource: parse::<ResourceId>("imported resource's source", &source)?,
        resource_name: stored_name("imported resource's source name", &source_name)?,
        version: version
          .map(|number| {
            NonZeroU32::new(number)
              .map(VersionNumber::new)
              .ok_or_else(|| corrupt("imported resource's source version", number))
          })
          .transpose()?,
      };
      Ok((parse::<ResourceId>("imported resource", &id)?, provenance))
    })
    .collect()
}

/// What the import with `key` did with each source item it did not copy, in plan order.
fn resolutions(connection: &Connection, key: &str) -> Result<Vec<Resolved>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT source_kind, source_id, source_name, target_id FROM import_resolutions \
     WHERE import_key = ?1 ORDER BY position",
  )?;
  let rows = statement.query_map([key], |row| {
    Ok((
      row.get::<_, String>(0)?,
      row.get::<_, String>(1)?,
      row.get::<_, String>(2)?,
      row.get::<_, Option<String>>(3)?,
    ))
  })?;
  rows
    .map(|row| {
      let (kind, source, source_name, target) = row?;
      Ok(Resolved {
        source: entity(&kind, &source)?,
        source_name,
        target: target.map(|target| entity(&kind, &target)).transpose()?,
      })
    })
    .collect()
}

/// An entity as its kind and id columns.
fn entity_columns(entity: Entity) -> (&'static str, String) {
  match entity {
    Entity::Resource(id) => ("resource", id.to_string()),
    Entity::Connection(id) => ("connection", id.to_string()),
    Entity::McpServer(id) => ("mcp_server", id.to_string()),
    Entity::Secret(id) => ("secret", id.to_string()),
  }
}

fn entity(kind: &str, id: &str) -> Result<Entity, StoreError> {
  const WHAT: &str = "import resolution";
  Ok(match kind {
    "resource" => Entity::Resource(parse::<ResourceId>(WHAT, id)?),
    "connection" => Entity::Connection(parse::<ConnectionId>(WHAT, id)?),
    "mcp_server" => Entity::McpServer(parse::<ConnectionId>(WHAT, id)?),
    "secret" => Entity::Secret(parse::<SecretId>(WHAT, id)?),
    other => return Err(corrupt(WHAT, format!("unknown kind {other}"))),
  })
}

fn stored_name(what: &'static str, name: &str) -> Result<Name, StoreError> {
  Name::new(name).map_err(|error| corrupt(what, error))
}

fn encode_content(content: &Content) -> Result<String, StoreError> {
  match content {
    Content::AgentProfile(body) => encode(body),
    Content::Instruction(body) => encode(body),
    Content::Pipeline(body) => encode(body),
    Content::Tool(body) => encode(body),
  }
}
