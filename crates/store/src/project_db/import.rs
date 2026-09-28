//! Import resources: the copies of another project's resources, written in one transaction, and
//! where each came from (`resource_imports`).

use orchester_core::{
  id::IdempotencyKey,
  import::{Content, CopiedResource, Entity, ImportBatch},
  resource::{Resource, VersionNumber},
  store::ImportStore,
};
use rusqlite::{Transaction, params};

use super::{
  ProjectDb,
  resources::{encode, kind_text, load_resources},
};
use crate::StoreError;

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
    transaction.commit()?;
    Ok(batch.copies().iter().map(|copy| copy.resource().clone()).collect())
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
     source_resource_id, source_version, imported_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![
      id,
      key.to_string(),
      provenance.project.to_string(),
      provenance.project_name.as_str(),
      provenance.resource.to_string(),
      provenance.version.map(VersionNumber::get),
      created_at
    ],
  )?;
  Ok(())
}

fn encode_content(content: &Content) -> Result<String, StoreError> {
  match content {
    Content::AgentProfile(body) => encode(body),
    Content::Instruction(body) => encode(body),
    Content::Pipeline(body) => encode(body),
    Content::Tool(body) => encode(body),
  }
}
