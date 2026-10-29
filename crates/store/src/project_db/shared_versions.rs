//! The cache of a shared project's configuration (`docs/architecture/server.md`, "Desktop data", S5): which resources
//! are the server's, and each version the server confirmed with its publisher, kept twice — the server's body
//! unchanged, with its hash and references, and the local body with this computer's bindings in place of the
//! references, which runs and editors read ([`super::resources`]).

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ResourceId, SecretId, VersionId},
  resource::{ResourceBody, Version, VersionNumber},
  sharing::{SharedPerson, TaskRevision},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{
  ProjectDb,
  resources::{check_kind, decode, encode, load_versions},
};
use crate::{
  StoreError,
  rows::{corrupt, parse, tokens_column},
};

/// Whether a resource of a shared project is the server's; a local project's resources have no mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceSharing {
  /// The server's resource, at its revision there.
  Shared(TaskRevision),
  /// Kept on this computer only when the project joined a server project.
  NotShared,
}

/// What a version's reference names on the publisher's computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceTarget {
  Connection(ConnectionId),
  McpServer(ConnectionId),
  Secret(SecretId),
  Tool(ResourceId),
}

/// One id a version's body names, with what each computer binds it by: its name and its role there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionReference {
  pub target: ReferenceTarget,
  pub name: String,
  pub role: Option<String>,
}

/// A version of a shared resource as the server confirmed it, with its local body.
#[derive(Clone, Debug, PartialEq)]
pub struct CachedVersion<B> {
  pub number: VersionNumber,
  pub published_at: Timestamp,
  pub publisher: SharedPerson,
  /// The body as the server keeps it, never changed here.
  pub server_body: serde_json::Value,
  pub hash: ContentHash,
  pub references: Vec<VersionReference>,
  /// The body with the local connections, MCP servers and secrets the references are bound to here.
  pub local_body: B,
}

impl ProjectDb {
  /// Marks resource `id` as the server's or as kept local, or clears the mark (`None`) when the project is local again.
  pub fn set_resource_sharing(&mut self, id: ResourceId, sharing: Option<ResourceSharing>) -> Result<(), StoreError> {
    let (text, revision) = match sharing {
      Some(ResourceSharing::Shared(revision)) => (
        Some("shared"),
        Some(tokens_column("resource revision", revision.get())?),
      ),
      Some(ResourceSharing::NotShared) => (Some("not_shared"), None),
      None => (None, None),
    };
    let changed = self.connection.execute(
      "UPDATE resources SET sharing = ?2, server_revision = ?3 WHERE id = ?1",
      params![id.to_string(), text, revision],
    )?;
    if changed == 0 {
      return Err(StoreError::ResourceNotFound(id));
    }
    Ok(())
  }

  pub fn resource_sharing(&self, id: ResourceId) -> Result<Option<ResourceSharing>, StoreError> {
    let row: Option<(Option<String>, Option<i64>)> = self
      .connection
      .query_row(
        "SELECT sharing, server_revision FROM resources WHERE id = ?1",
        [id.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    match row.ok_or(StoreError::ResourceNotFound(id))? {
      (None, _) => Ok(None),
      (Some(sharing), revision) => match (sharing.as_str(), revision) {
        ("shared", Some(revision)) => Ok(Some(ResourceSharing::Shared(revision_value(revision)?))),
        ("not_shared", None) => Ok(Some(ResourceSharing::NotShared)),
        _ => Err(corrupt("resource sharing", sharing)),
      },
    }
  }

  /// Keeps `version` of `resource` as the server confirmed it. A version this computer has already, e.g. one published
  /// here before the project was shared, keeps its row and gets the publisher; its bodies and references are replaced.
  pub fn cache_shared_version<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    version: &CachedVersion<B>,
  ) -> Result<Version<B>, StoreError> {
    let local_body = encode(&version.local_body)?;
    let transaction = self.connection.transaction()?;
    check_kind::<B>(&transaction, resource)?;
    let id = write_version(&transaction, resource, version, &local_body)?;
    transaction.execute(
      "INSERT INTO shared_version_bodies (version_id, server_body, sha256, local_body) VALUES (?1, ?2, ?3, ?4) \
       ON CONFLICT (version_id) DO UPDATE SET server_body = excluded.server_body, sha256 = excluded.sha256, \
       local_body = excluded.local_body",
      params![
        id.to_string(),
        version.server_body.to_string(),
        version.hash.as_str(),
        local_body
      ],
    )?;
    write_references(&transaction, id, &version.references)?;
    let cached = load_versions::<B>(&transaction, "WHERE id = ?1", [id.to_string()])?
      .pop()
      .ok_or_else(|| corrupt("resource version", id))?;
    transaction.commit()?;
    Ok(cached)
  }

  /// Version `number` of `resource` as the server confirmed it; `None` for a version that is not cached from a server.
  pub fn cached_version<B: ResourceBody>(
    &self,
    resource: ResourceId,
    number: VersionNumber,
  ) -> Result<Option<CachedVersion<B>>, StoreError> {
    check_kind::<B>(&self.connection, resource)?;
    let row: Option<CachedRow> = self
      .connection
      .query_row(
        "SELECT id, published_at, publisher_id, publisher_name, server_body, sha256, local_body \
         FROM resource_versions JOIN shared_version_bodies ON version_id = id \
         WHERE resource_id = ?1 AND number = ?2",
        params![resource.to_string(), number.get()],
        |row| {
          Ok(CachedRow {
            id: row.get(0)?,
            published_at: row.get(1)?,
            publisher: (row.get(2)?, row.get(3)?),
            server_body: row.get(4)?,
            hash: row.get(5)?,
            local_body: row.get(6)?,
          })
        },
      )
      .optional()?;
    let Some(row) = row else {
      return Ok(None);
    };
    let (Some(publisher_id), Some(publisher_name)) = row.publisher else {
      return Err(corrupt("version publisher", row.id));
    };
    Ok(Some(CachedVersion {
      number,
      published_at: Timestamp::from_unix_millis(row.published_at),
      publisher: SharedPerson {
        id: parse("publisher id", &publisher_id)?,
        name: publisher_name,
      },
      server_body: serde_json::from_str(&row.server_body).map_err(|error| corrupt("server body", error))?,
      hash: ContentHash::new(&row.hash).map_err(|error| corrupt("version hash", error))?,
      references: load_references(&self.connection, &row.id)?,
      local_body: decode(&row.local_body)?,
    }))
  }

  /// Replaces the local body of cached version `number` of `resource`, e.g. when a reference it names is bound anew;
  /// the server's body stays as it is. `false` when that version is not cached from a server.
  pub fn rewrite_local_body<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    number: VersionNumber,
    body: &B,
  ) -> Result<bool, StoreError> {
    check_kind::<B>(&self.connection, resource)?;
    let changed = self.connection.execute(
      "UPDATE shared_version_bodies SET local_body = ?3 WHERE version_id = \
       (SELECT id FROM resource_versions WHERE resource_id = ?1 AND number = ?2)",
      params![resource.to_string(), number.get(), encode(body)?],
    )?;
    Ok(changed == 1)
  }
}

struct CachedRow {
  id: String,
  published_at: i64,
  publisher: (Option<String>, Option<String>),
  server_body: String,
  hash: String,
  local_body: String,
}

/// The id of `version`'s row: the one this computer has for its number, given the publisher if it had none, or a new
/// one.
fn write_version<B>(
  connection: &Connection,
  resource: ResourceId,
  version: &CachedVersion<B>,
  local_body: &str,
) -> Result<VersionId, StoreError> {
  let publisher = &version.publisher;
  let existing: Option<String> = connection
    .query_row(
      "SELECT id FROM resource_versions WHERE resource_id = ?1 AND number = ?2",
      params![resource.to_string(), version.number.get()],
      |row| row.get(0),
    )
    .optional()?;
  if let Some(existing) = existing {
    connection.execute(
      "UPDATE resource_versions SET publisher_id = ?2, publisher_name = ?3 WHERE id = ?1 AND publisher_id IS NULL",
      params![existing, publisher.id.to_string(), publisher.name],
    )?;
    return parse("version id", &existing);
  }
  let id = VersionId::generate();
  connection.execute(
    "INSERT INTO resource_versions (id, resource_id, number, body, published_at, publisher_id, publisher_name) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![
      id.to_string(),
      resource.to_string(),
      version.number.get(),
      local_body,
      version.published_at.unix_millis(),
      publisher.id.to_string(),
      publisher.name
    ],
  )?;
  Ok(id)
}

fn write_references(
  connection: &Connection,
  version: VersionId,
  references: &[VersionReference],
) -> Result<(), StoreError> {
  connection.execute(
    "DELETE FROM shared_version_references WHERE version_id = ?1",
    [version.to_string()],
  )?;
  for (position, reference) in references.iter().enumerate() {
    let (kind, target) = match reference.target {
      ReferenceTarget::Connection(id) => ("connection", id.to_string()),
      ReferenceTarget::McpServer(id) => ("mcp_server", id.to_string()),
      ReferenceTarget::Secret(id) => ("secret", id.to_string()),
      ReferenceTarget::Tool(id) => ("tool", id.to_string()),
    };
    connection.execute(
      "INSERT INTO shared_version_references (version_id, position, kind, target_id, name, role) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![
        version.to_string(),
        i64::try_from(position).map_err(|_| StoreError::Unrepresentable("reference position"))?,
        kind,
        target,
        reference.name,
        reference.role
      ],
    )?;
  }
  Ok(())
}

fn load_references(connection: &Connection, version: &str) -> Result<Vec<VersionReference>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT kind, target_id, name, role FROM shared_version_references WHERE version_id = ?1 ORDER BY position",
  )?;
  let rows = statement
    .query_map([version], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, Option<String>>(3)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(kind, target, name, role)| {
      let target = match kind.as_str() {
        "connection" => ReferenceTarget::Connection(parse("reference id", &target)?),
        "mcp_server" => ReferenceTarget::McpServer(parse("reference id", &target)?),
        "secret" => ReferenceTarget::Secret(parse("reference id", &target)?),
        "tool" => ReferenceTarget::Tool(parse("reference id", &target)?),
        _ => return Err(corrupt("reference kind", kind)),
      };
      Ok(VersionReference { target, name, role })
    })
    .collect()
}

fn revision_value(revision: i64) -> Result<TaskRevision, StoreError> {
  u64::try_from(revision)
    .ok()
    .and_then(|revision| TaskRevision::try_from(revision).ok())
    .ok_or_else(|| corrupt("resource revision", revision))
}
