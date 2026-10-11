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

impl ProjectDb {
  /// Reads the draft and latest version under the write lock, then saves the returned draft.
  /// A rejected edit leaves storage unchanged; no competing writer can run between the read and save.
  pub fn update_draft<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    edit: impl FnOnce(Option<Draft<B>>, Option<Version<B>>) -> Result<Draft<B>, StoreError>,
  ) -> Result<DraftSaved, StoreError> {
    let transaction = self
      .connection
      .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    check_kind::<B>(&transaction, resource)?;
    let draft = load_draft::<B>(&transaction, resource)?;
    let latest = load_versions::<B>(
      &transaction,
      "WHERE resource_id = ?1 ORDER BY number DESC LIMIT 1",
      [resource.to_string()],
    )?
    .pop();
    let changed = edit(draft, latest)?;
    let saved = save_draft(&transaction, resource, &changed)?;
    transaction.commit()?;
    Ok(saved)
  }

  /// Saves only over the body and latest version the editor read, checked inside the write transaction.
  pub fn save_draft_matching<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    expected: &B,
    latest: Option<VersionNumber>,
    draft: &Draft<B>,
  ) -> Result<DraftSaved, StoreError> {
    self.save_draft_and_name_matching(resource, expected, latest, draft, None)
  }

  /// Saves an editor draft and an optional display-name change atomically; slugs never change.
  pub fn save_draft_and_name_matching<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    expected: &B,
    latest: Option<VersionNumber>,
    draft: &Draft<B>,
    names: Option<(&str, &Name)>,
  ) -> Result<DraftSaved, StoreError> {
    let transaction = self.connection.transaction()?;
    check_kind::<B>(&transaction, resource)?;
    if let Some((before, after)) = names {
      let changed = transaction.execute(
        "UPDATE resources SET name = ?1 WHERE id = ?2 AND name = ?3",
        params![after.as_str(), resource.to_string(), before],
      )?;
      if changed != 1 {
        return Err(StoreError::DraftChanged(resource));
      }
    }
    let stored = load_draft::<B>(&transaction, resource)?;
    let published = load_versions::<B>(
      &transaction,
      "WHERE resource_id = ?1 ORDER BY number DESC LIMIT 1",
      [resource.to_string()],
    )?
    .pop();
    let current = stored
      .as_ref()
      .map(Draft::body)
      .or_else(|| published.as_ref().map(Version::body));
    if current != Some(expected) || published.as_ref().map(Version::number) != latest {
      return Err(StoreError::DraftChanged(resource));
    }
    let saved = save_draft(&transaction, resource, draft)?;
    transaction.commit()?;
    Ok(saved)
  }
  /// Publishes the draft of `resource` only if its body is `expected`, in the same transaction that reads
  /// it: a check made on the draft holds for what is published ([`StoreError::DraftChanged`] otherwise).
  pub fn publish_draft_matching<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    expected: &B,
    now: Timestamp,
  ) -> Result<Version<B>, StoreError> {
    self.publish_checked(resource, Some(expected), None, now)
  }

  /// Also pins the latest version against which a permission review was made.
  pub fn publish_draft_matching_version<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    expected: &B,
    latest: Option<VersionNumber>,
    now: Timestamp,
  ) -> Result<Version<B>, StoreError> {
    self.publish_checked(resource, Some(expected), Some(latest), now)
  }

  fn publish_checked<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    expected: Option<&B>,
    expected_latest: Option<Option<VersionNumber>>,
    now: Timestamp,
  ) -> Result<Version<B>, StoreError> {
    let transaction = self.connection.transaction()?;
    let version = publish_checked(&transaction, resource, expected, expected_latest, now)?;
    transaction.commit()?;
    Ok(version)
  }

  /// Validates or rebases the draft under the publishing transaction. A rejected candidate
  /// changes neither the draft nor the versions; concurrent writers cannot change the reviewed base.
  pub fn publish_draft_with<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    now: Timestamp,
    prepare: impl FnOnce(Draft<B>, Option<Version<B>>) -> Result<Draft<B>, StoreError>,
  ) -> Result<Version<B>, StoreError> {
    let transaction = self
      .connection
      .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    check_kind::<B>(&transaction, resource)?;
    let draft = load_draft::<B>(&transaction, resource)?.ok_or(StoreError::NoDraft(resource))?;
    let latest = load_versions::<B>(
      &transaction,
      "WHERE resource_id = ?1 ORDER BY number DESC LIMIT 1",
      [resource.to_string()],
    )?
    .pop();
    let number = latest.as_ref().map(Version::number);
    let draft = prepare(draft, latest)?;
    let version = publish_prepared(&transaction, resource, draft, number, now)?;
    transaction.commit()?;
    Ok(version)
  }
}

/// A checked mutation of the resource snapshot read under the write lock.
pub enum ResourceUpdate<B> {
  SaveDraft(Draft<B>),
  PublishDraft(Timestamp),
}

impl ProjectDb {
  /// Reads the resource, its draft and latest version under an immediate write lock, then applies
  /// `update` in that transaction. A rejected snapshot leaves the resource unchanged.
  pub fn update_resource<B: ResourceBody, E: From<StoreError>>(
    &mut self,
    id: ResourceId,
    update: impl FnOnce(Resource, Option<Draft<B>>, Option<Version<B>>) -> Result<ResourceUpdate<B>, E>,
  ) -> Result<(), E> {
    let transaction = self
      .connection
      .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
      .map_err(StoreError::from)?;
    check_kind::<B>(&transaction, id)?;
    let resource = load_resources(&transaction, "WHERE id = ?1", [id.to_string()])?
      .pop()
      .ok_or(StoreError::ResourceNotFound(id))?;
    let draft = load_draft::<B>(&transaction, id)?;
    let latest = load_versions::<B>(
      &transaction,
      "WHERE resource_id = ?1 ORDER BY number DESC LIMIT 1",
      [id.to_string()],
    )?
    .pop();
    match update(resource, draft, latest)? {
      ResourceUpdate::SaveDraft(draft) => {
        save_draft(&transaction, id, &draft)?;
      }
      ResourceUpdate::PublishDraft(now) => {
        publish_checked::<B>(&transaction, id, None, None, now)?;
      }
    }
    transaction.commit().map_err(StoreError::from)?;
    Ok(())
  }
}

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
    let transaction = self.connection.transaction()?;
    let saved = save_draft(&transaction, resource, draft)?;
    transaction.commit()?;
    Ok(saved)
  }

  fn draft<B: ResourceBody>(&self, resource: ResourceId) -> Result<Option<Draft<B>>, StoreError> {
    check_kind::<B>(&self.connection, resource)?;
    load_draft(&self.connection, resource)
  }

  fn publish_draft<B: ResourceBody>(&mut self, resource: ResourceId, now: Timestamp) -> Result<Version<B>, StoreError> {
    self.publish_checked(resource, None, None, now)
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

  fn versions<B: ResourceBody>(&self, resource: ResourceId) -> Result<Vec<Version<B>>, StoreError> {
    check_kind::<B>(&self.connection, resource)?;
    load_versions(
      &self.connection,
      "WHERE resource_id = ?1 ORDER BY number",
      [resource.to_string()],
    )
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

fn publish_checked<B: ResourceBody>(
  connection: &Connection,
  resource: ResourceId,
  expected: Option<&B>,
  expected_latest: Option<Option<VersionNumber>>,
  now: Timestamp,
) -> Result<Version<B>, StoreError> {
  check_kind::<B>(connection, resource)?;
  let draft = load_draft::<B>(connection, resource)?.ok_or(StoreError::NoDraft(resource))?;
  if expected.is_some_and(|expected| draft.body() != expected) {
    return Err(StoreError::DraftChanged(resource));
  }
  draft.body().check_publishable()?;
  let latest = load_versions::<B>(
    connection,
    "WHERE resource_id = ?1 ORDER BY number DESC LIMIT 1",
    [resource.to_string()],
  )?
  .pop()
  .map(|version| version.number());
  if expected_latest.is_some_and(|expected| expected != latest) {
    return Err(StoreError::DraftChanged(resource));
  }
  publish_prepared(connection, resource, draft, latest, now)
}

fn publish_prepared<B: ResourceBody>(
  connection: &Connection,
  resource: ResourceId,
  draft: Draft<B>,
  latest: Option<VersionNumber>,
  now: Timestamp,
) -> Result<Version<B>, StoreError> {
  draft.body().check_publishable()?;
  let version = publish(draft, latest, now)?;
  connection.execute(
    "INSERT INTO resource_versions (id, resource_id, number, body, published_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![
      version.id().to_string(),
      resource.to_string(),
      version.number().get(),
      encode(version.body())?,
      now.unix_millis()
    ],
  )?;
  connection.execute(
    "DELETE FROM resource_drafts WHERE resource_id = ?1",
    [resource.to_string()],
  )?;
  Ok(version)
}

fn save_draft<B: ResourceBody>(
  connection: &Connection,
  resource: ResourceId,
  draft: &Draft<B>,
) -> Result<DraftSaved, StoreError> {
  let body = encode(draft.body())?;
  check_kind::<B>(connection, resource)?;
  let base = match draft.based_on() {
    Some(number) => load_versions::<B>(
      connection,
      "WHERE resource_id = ?1 AND number = ?2",
      params![resource.to_string(), number.get()],
    )?
    .pop(),
    None => None,
  };
  if base.is_some_and(|version| draft.repeats(&version)) {
    connection.execute(
      "DELETE FROM resource_drafts WHERE resource_id = ?1",
      [resource.to_string()],
    )?;
    return Ok(DraftSaved::Discarded);
  }
  connection.execute(
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
  Ok(DraftSaved::Stored)
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

pub(super) fn load_resources(
  connection: &Connection,
  clause: &str,
  params: impl Params,
) -> Result<Vec<Resource>, StoreError> {
  let mut statement = connection.prepare(&format!("{SELECT_RESOURCES} {clause}"))?;
  let rows = statement
    .query_map(params, ResourceRow::read)?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(ResourceRow::into_resource).collect()
}

pub(super) fn load_draft<B: ResourceBody>(
  connection: &Connection,
  resource: ResourceId,
) -> Result<Option<Draft<B>>, StoreError> {
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

pub(super) fn load_versions<B: ResourceBody>(
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

pub(super) fn encode<B: ResourceBody>(body: &B) -> Result<String, StoreError> {
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

pub(crate) fn version_number(number: i64) -> Result<VersionNumber, StoreError> {
  u32::try_from(number)
    .ok()
    .and_then(NonZeroU32::new)
    .map(VersionNumber::new)
    .ok_or_else(|| corrupt("version number", number))
}

pub(super) const fn kind_text(kind: ResourceKind) -> &'static str {
  match kind {
    ResourceKind::AgentProfile => "agent_profile",
    ResourceKind::Instruction => "instruction",
    ResourceKind::Pipeline => "pipeline",
    ResourceKind::Tool => "tool",
    ResourceKind::Skill => "skill",
  }
}

fn parse_kind(value: &str) -> Result<ResourceKind, StoreError> {
  match value {
    "agent_profile" => Ok(ResourceKind::AgentProfile),
    "instruction" => Ok(ResourceKind::Instruction),
    "pipeline" => Ok(ResourceKind::Pipeline),
    "tool" => Ok(ResourceKind::Tool),
    "skill" => Ok(ResourceKind::Skill),
    other => Err(corrupt("resource kind", other)),
  }
}
