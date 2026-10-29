//! The cache of a shared project's configuration (`docs/architecture/server.md`, "Desktop data", S5): which resources
//! are the server's, and each version the server confirmed with its publisher, kept twice — the server's body
//! unchanged, with its hash and references, and the local body with this computer's bindings in place of the
//! references, which runs and editors read ([`super::resources`]).

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ResourceId, SecretId, VersionId},
  resource::{Resource, ResourceBody, ResourceKind, Version, VersionNumber},
  sharing::{SharedPerson, TaskRevision},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{
  ProjectDb,
  resources::{
    check_kind, decode, encode, kind_text, load_resources, load_versions, newest_number, parse_kind, version_number,
  },
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
  /// A teammate's version that widens what agents may do here: the latest version reads leave it out, so runs keep
  /// the version before it, until the person's *Use v<N> here* ([`ProjectDb::use_shared_version`]).
  pub waits_here: bool,
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
  /// A version that does not wait here supersedes the versions before it that did.
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
    if version.waits_here {
      transaction.execute(
        "INSERT OR IGNORE INTO waiting_versions (version_id) VALUES (?1)",
        [id.to_string()],
      )?;
    } else {
      stop_waiting(&transaction, resource, version.number)?;
    }
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
        "SELECT id, published_at, publisher_id, publisher_name, server_body, sha256, local_body, \
         EXISTS (SELECT 1 FROM waiting_versions WHERE waiting_versions.version_id = id) \
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
            waits_here: row.get(7)?,
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
      waits_here: row.waits_here,
    }))
  }

  /// The person's *Use v<N> here*: version `number` of `resource`, which waited, is the one runs use from now on, and
  /// the versions before it wait no more. `false` when it did not wait. Only a person does this (row 35); the store
  /// does not know who asks, so its caller refuses everyone else.
  pub fn use_shared_version(&mut self, resource: ResourceId, number: VersionNumber) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let waited: bool = transaction.query_row(
      "SELECT EXISTS (SELECT 1 FROM waiting_versions JOIN resource_versions ON resource_versions.id = version_id \
       WHERE resource_id = ?1 AND number = ?2)",
      params![resource.to_string(), number.get()],
      |row| row.get(0),
    )?;
    if waited {
      stop_waiting(&transaction, resource, number)?;
    }
    transaction.commit()?;
    Ok(waited)
  }

  /// The newest version of `resource` that waits for the person's *Use v<N> here*; `None` when none waits.
  pub fn waiting_version(&self, resource: ResourceId) -> Result<Option<VersionNumber>, StoreError> {
    let number: Option<i64> = self.connection.query_row(
      "SELECT MAX(number) FROM resource_versions JOIN waiting_versions ON version_id = id WHERE resource_id = ?1",
      [resource.to_string()],
      |row| row.get(0),
    )?;
    number.map(version_number).transpose()
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

  /// Keeps `resource` as the server's at `revision`. One this computer does not have is added with the server's id and
  /// no draft (its versions come with [`Self::cache_shared_version`]); one it has takes the server's name. Another
  /// local resource of its kind with its slug refuses it ([`StoreError::DuplicateSlug`]).
  pub fn keep_shared_resource(&mut self, resource: &Resource, revision: TaskRevision) -> Result<(), StoreError> {
    let id = resource.id();
    let revision = tokens_column("resource revision", revision.get())?;
    let transaction = self.connection.transaction()?;
    match load_resources(&transaction, "WHERE id = ?1", [id.to_string()])?.pop() {
      Some(kept) if kept.kind() != resource.kind() => {
        return Err(StoreError::ResourceKindMismatch {
          resource: id,
          expected: resource.kind(),
          found: kept.kind(),
        });
      }
      Some(_) => {
        transaction.execute(
          "UPDATE resources SET name = ?2 WHERE id = ?1",
          params![id.to_string(), resource.name().as_str()],
        )?;
      }
      None => insert_resource(&transaction, resource)?,
    }
    transaction.execute(
      "UPDATE resources SET sharing = 'shared', server_revision = ?2 WHERE id = ?1",
      params![id.to_string(), revision],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// The number of the newest version this computer has of `resource`, cached from a server or published here.
  pub fn latest_version_number(&self, resource: ResourceId) -> Result<Option<VersionNumber>, StoreError> {
    newest_number(&self.connection, resource)
  }

  /// The cached versions whose references name `target`, by resource and number.
  pub fn versions_naming(&self, target: ReferenceTarget) -> Result<Vec<NamingVersion>, StoreError> {
    let (kind, id) = target_columns(target);
    let mut statement = self.connection.prepare(
      "SELECT resources.id, resources.kind, resource_versions.number FROM shared_version_references \
       JOIN resource_versions ON resource_versions.id = shared_version_references.version_id \
       JOIN resources ON resources.id = resource_versions.resource_id \
       WHERE shared_version_references.kind = ?1 AND shared_version_references.target_id = ?2 \
       ORDER BY resources.rowid, resource_versions.number",
    )?;
    let rows = statement
      .query_map(params![kind, id], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, i64>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(resource, kind, number)| {
        Ok(NamingVersion {
          resource: parse("resource id", &resource)?,
          kind: parse_kind(&kind)?,
          number: version_number(number)?,
        })
      })
      .collect()
  }
}

/// A cached version whose references name a given connection, MCP server or secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NamingVersion {
  pub resource: ResourceId,
  pub kind: ResourceKind,
  pub number: VersionNumber,
}

/// Adds `resource` with its own id and no draft, unless the slug is another resource's of its kind.
fn insert_resource(connection: &Connection, resource: &Resource) -> Result<(), StoreError> {
  let kind = kind_text(resource.kind());
  let taken: bool = connection.query_row(
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
  connection.execute(
    "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![
      resource.id().to_string(),
      kind,
      resource.slug().as_str(),
      resource.name().as_str(),
      resource.created_at().unix_millis()
    ],
  )?;
  Ok(())
}

struct CachedRow {
  id: String,
  published_at: i64,
  publisher: (Option<String>, Option<String>),
  server_body: String,
  hash: String,
  local_body: String,
  waits_here: bool,
}

/// Version `number` of `resource` and the versions before it wait no more.
pub(super) fn stop_waiting(
  connection: &Connection,
  resource: ResourceId,
  number: VersionNumber,
) -> Result<(), StoreError> {
  connection.execute(
    "DELETE FROM waiting_versions WHERE version_id IN \
     (SELECT id FROM resource_versions WHERE resource_id = ?1 AND number <= ?2)",
    params![resource.to_string(), number.get()],
  )?;
  Ok(())
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
    let (kind, target) = target_columns(reference.target);
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

/// The `kind` and `target_id` columns of a reference.
fn target_columns(target: ReferenceTarget) -> (&'static str, String) {
  match target {
    ReferenceTarget::Connection(id) => ("connection", id.to_string()),
    ReferenceTarget::McpServer(id) => ("mcp_server", id.to_string()),
    ReferenceTarget::Secret(id) => ("secret", id.to_string()),
    ReferenceTarget::Tool(id) => ("tool", id.to_string()),
  }
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
