use std::num::NonZeroU32;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, DraftSaved, Resource, ResourceBody, ResourceKind, Slug, Version, VersionNumber, publish},
  store::ResourceStore,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Params, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_body, decode_document, encode_json, parse},
};

const SELECT_RESOURCES: &str = "SELECT id, kind, slug, name, created_at FROM resources";
const SELECT_VERSIONS: &str = "SELECT id, number, body, published_at FROM resource_versions";

impl ResourceStore for ProjectDb {
  type Error = StoreError;

  fn create_resource<B: ResourceBody>(
    &mut self,
    slug: Slug,
    name: Name,
    body: &B,
    now: Timestamp,
  ) -> Result<Resource, StoreError> {
    let body = encode(body)?;
    let transaction = self.connection.transaction()?;
    let taken: bool = transaction.query_row(
      "SELECT EXISTS (SELECT 1 FROM resources WHERE kind = ?1 AND slug = ?2)",
      params![kind_text(B::KIND), slug.as_str()],
      |row| row.get(0),
    )?;
    if taken {
      return Err(StoreError::DuplicateSlug {
        kind: B::KIND,
        slug: slug.as_str().to_owned(),
      });
    }
    let resource = Resource::new(ResourceId::generate(), B::KIND, slug, name, now);
    let id = resource.id().to_string();
    transaction.execute(
      "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        id,
        kind_text(B::KIND),
        resource.slug().as_str(),
        resource.name().as_str(),
        now.unix_millis()
      ],
    )?;
    transaction.execute(
      "INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) VALUES (?1, ?2, NULL, ?3)",
      params![id, body, now.unix_millis()],
    )?;
    transaction.commit()?;
    Ok(resource)
  }

  fn resource(&self, id: ResourceId) -> Result<Option<Resource>, StoreError> {
    let mut found = load_resources(&self.connection, "WHERE id = ?1", [id.to_string()])?;
    Ok(found.pop())
  }

  fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, StoreError> {
    load_resources(&self.connection, "WHERE kind = ?1 ORDER BY slug", [kind_text(kind)])
  }

  fn save_draft<B: ResourceBody>(&mut self, resource: ResourceId, draft: &Draft<B>) -> Result<DraftSaved, StoreError> {
    let body = encode(draft.body())?;
    let transaction = self.connection.transaction()?;
    check_kind::<B>(&transaction, resource)?;
    let base = match draft.based_on() {
      Some(number) => load_versions::<B>(
        &transaction,
        "WHERE resource_id = ?1 AND number = ?2",
        params![resource.to_string(), number.get()],
      )?
      .pop(),
      None => None,
    };
    if base.is_some_and(|version| draft.repeats(&version)) {
      transaction.execute(
        "DELETE FROM resource_drafts WHERE resource_id = ?1",
        [resource.to_string()],
      )?;
      transaction.commit()?;
      return Ok(DraftSaved::Discarded);
    }
    transaction.execute(
      "INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) VALUES (?1, ?2, ?3, ?4) \
       ON CONFLICT (resource_id) DO UPDATE SET body = excluded.body, based_on = excluded.based_on, \
       updated_at = excluded.updated_at",
      params![
        resource.to_string(),
        body,
        draft.based_on().map(VersionNumber::get),
        draft.updated_at().unix_millis()
      ],
    )?;
    transaction.commit()?;
    Ok(DraftSaved::Stored)
  }

  fn draft<B: ResourceBody>(&self, resource: ResourceId) -> Result<Option<Draft<B>>, StoreError> {
    check_kind::<B>(&self.connection, resource)?;
    load_draft(&self.connection, resource)
  }

  fn publish_draft<B: ResourceBody>(&mut self, resource: ResourceId, now: Timestamp) -> Result<Version<B>, StoreError> {
    let transaction = self.connection.transaction()?;
    check_kind::<B>(&transaction, resource)?;
    let draft = load_draft::<B>(&transaction, resource)?.ok_or(StoreError::NoDraft(resource))?;
    draft.body().check_publishable()?;
    let latest = load_versions::<B>(
      &transaction,
      "WHERE resource_id = ?1 ORDER BY number DESC LIMIT 1",
      [resource.to_string()],
    )?
    .pop()
    .map(|version| version.number());
    let version = publish(draft, latest, now)?;
    transaction.execute(
      "INSERT INTO resource_versions (id, resource_id, number, body, published_at) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        version.id().to_string(),
        resource.to_string(),
        version.number().get(),
        encode(version.body())?,
        now.unix_millis()
      ],
    )?;
    transaction.execute(
      "DELETE FROM resource_drafts WHERE resource_id = ?1",
      [resource.to_string()],
    )?;
    transaction.commit()?;
    Ok(version)
  }

  fn latest_version<B: ResourceBody>(&self, resource: ResourceId) -> Result<Option<Version<B>>, StoreError> {
    check_kind::<B>(&self.connection, resource)?;
    let mut found = load_versions(
      &self.connection,
      "WHERE resource_id = ?1 ORDER BY number DESC LIMIT 1",
      [resource.to_string()],
    )?;
    Ok(found.pop())
  }

  fn version<B: ResourceBody>(
    &self,
    resource: ResourceId,
    number: VersionNumber,
  ) -> Result<Option<Version<B>>, StoreError> {
    check_kind::<B>(&self.connection, resource)?;
    let mut found = load_versions(
      &self.connection,
      "WHERE resource_id = ?1 AND number = ?2",
      params![resource.to_string(), number.get()],
    )?;
    Ok(found.pop())
  }
}

/// Fails unless `resource` exists and holds bodies of type `B`.
fn check_kind<B: ResourceBody>(connection: &Connection, resource: ResourceId) -> Result<(), StoreError> {
  let kind: Option<String> = connection
    .query_row(
      "SELECT kind FROM resources WHERE id = ?1",
      [resource.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  let found = parse_kind(&kind.ok_or(StoreError::ResourceNotFound(resource))?)?;
  if found == B::KIND {
    Ok(())
  } else {
    Err(StoreError::ResourceKindMismatch {
      resource,
      expected: B::KIND,
      found,
    })
  }
}

fn load_resources(connection: &Connection, clause: &str, params: impl Params) -> Result<Vec<Resource>, StoreError> {
  let mut statement = connection.prepare(&format!("{SELECT_RESOURCES} {clause}"))?;
  let rows = statement
    .query_map(params, ResourceRow::read)?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(ResourceRow::into_resource).collect()
}

fn load_draft<B: ResourceBody>(connection: &Connection, resource: ResourceId) -> Result<Option<Draft<B>>, StoreError> {
  let row: Option<(String, Option<i64>, i64)> = connection
    .query_row(
      "SELECT body, based_on, updated_at FROM resource_drafts WHERE resource_id = ?1",
      [resource.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .optional()?;
  let Some((body, based_on, updated_at)) = row else {
    return Ok(None);
  };
  Ok(Some(Draft::new(
    decode(&body)?,
    based_on.map(version_number).transpose()?,
    Timestamp::from_unix_millis(updated_at),
  )))
}

fn load_versions<B: ResourceBody>(
  connection: &Connection,
  clause: &str,
  params: impl Params,
) -> Result<Vec<Version<B>>, StoreError> {
  let mut statement = connection.prepare(&format!("{SELECT_VERSIONS} {clause}"))?;
  let rows = statement
    .query_map(params, |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, i64>(3)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(id, number, body, published_at)| {
      Ok(Version::new(
        parse("version id", &id)?,
        version_number(number)?,
        decode(&body)?,
        Timestamp::from_unix_millis(published_at),
      ))
    })
    .collect()
}

struct ResourceRow {
  id: String,
  kind: String,
  slug: String,
  name: String,
  created_at: i64,
}

impl ResourceRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      kind: row.get(1)?,
      slug: row.get(2)?,
      name: row.get(3)?,
      created_at: row.get(4)?,
    })
  }

  fn into_resource(self) -> Result<Resource, StoreError> {
    Ok(Resource::new(
      parse("resource id", &self.id)?,
      parse_kind(&self.kind)?,
      Slug::new(&self.slug).map_err(|error| corrupt("resource slug", error))?,
      Name::new(&self.name).map_err(|error| corrupt("resource name", error))?,
      Timestamp::from_unix_millis(self.created_at),
    ))
  }
}

fn encode<B: ResourceBody>(body: &B) -> Result<String, StoreError> {
  encode_json("resource body", B::SCHEMA, body)
}

/// A stored body in the current shape; bodies of an older schema are upgraded as they are read
/// (published versions are immutable, so they are not rewritten).
fn decode<B: ResourceBody>(text: &str) -> Result<B, StoreError> {
  const WHAT: &str = "resource body";
  let (schema, body) = decode_document(WHAT, text)?;
  if schema == B::SCHEMA {
    return decode_body(WHAT, body);
  }
  let upgraded = (schema < B::SCHEMA)
    .then(|| B::upgrade(schema, body))
    .flatten()
    .ok_or_else(|| {
      corrupt(
        WHAT,
        format!("schema {schema} is not the supported schema {}", B::SCHEMA),
      )
    })?;
  decode_body(WHAT, upgraded)
}

fn version_number(number: i64) -> Result<VersionNumber, StoreError> {
  u32::try_from(number)
    .ok()
    .and_then(NonZeroU32::new)
    .map(VersionNumber::new)
    .ok_or_else(|| corrupt("version number", number))
}

const fn kind_text(kind: ResourceKind) -> &'static str {
  match kind {
    ResourceKind::AgentProfile => "agent_profile",
    ResourceKind::Instruction => "instruction",
    ResourceKind::Pipeline => "pipeline",
    ResourceKind::Tool => "tool",
  }
}

fn parse_kind(value: &str) -> Result<ResourceKind, StoreError> {
  match value {
    "agent_profile" => Ok(ResourceKind::AgentProfile),
    "instruction" => Ok(ResourceKind::Instruction),
    "pipeline" => Ok(ResourceKind::Pipeline),
    "tool" => Ok(ResourceKind::Tool),
    other => Err(corrupt("resource kind", other)),
  }
}
