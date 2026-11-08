//! A new resource is created as its first version, with no draft (the decision of 2026-11-06): creating it publishes
//! v1 at once, and a draft is only ever saved over a version that exists. In a team project the version waits for the
//! server like every version published here ([`super::shared_publishes`]); one the server refuses is taken back whole
//! ([`ProjectDb::discard_created`]), so nothing stays as created.

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, Resource, ResourceBody, ResourceKind, Slug, Version},
  time::Timestamp,
};
use rusqlite::{Connection, params};

use super::{
  ProjectDb,
  resources::{kind_text, publish_prepared},
  shared_publishes,
};
use crate::StoreError;

/// Inserts a resource of kind `B::KIND` under `slug`, in the caller's transaction.
pub(super) fn insert<B: ResourceBody>(
  connection: &Connection,
  slug: Slug,
  name: Name,
  now: Timestamp,
) -> Result<Resource, StoreError> {
  shared_publishes::ensure_may_create(connection, B::KIND)?;
  let taken: bool = connection.query_row(
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
  connection.execute(
    "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![
      resource.id().to_string(),
      kind_text(B::KIND),
      resource.slug().as_str(),
      resource.name().as_str(),
      now.unix_millis()
    ],
  )?;
  Ok(resource)
}

/// Whether resources of `kind` are created as their first version: agent profiles, instructions, skills, tools and
/// MCP servers (the team's; a local project's are plain rows). A pipeline keeps its own flow, and agent testing's kinds
/// are not covered.
const fn created_published(kind: ResourceKind) -> bool {
  matches!(
    kind,
    ResourceKind::AgentProfile
      | ResourceKind::Instruction
      | ResourceKind::Skill
      | ResourceKind::Tool
      | ResourceKind::McpServer
  )
}

/// Fails when a draft of `resource` would be the first thing it holds: a resource of a kind created as its first
/// version has a draft only over a version. One that already has a draft (a copy imported as a draft) keeps being
/// edited.
pub(super) fn ensure_has_version<B: ResourceBody>(
  connection: &Connection,
  resource: ResourceId,
) -> Result<(), StoreError> {
  if !created_published(B::KIND) {
    return Ok(());
  }
  let held: bool = connection.query_row(
    "SELECT EXISTS (SELECT 1 FROM resource_versions WHERE resource_id = ?1) \
     OR EXISTS (SELECT 1 FROM resource_drafts WHERE resource_id = ?1)",
    [resource.to_string()],
    |row| row.get(0),
  )?;
  if held {
    Ok(())
  } else {
    Err(StoreError::NoFirstVersion(resource))
  }
}

impl ProjectDb {
  /// Creates a resource of kind `B::KIND` as its first version, atomically, under `slug` (unique among its kind): the
  /// resource has no draft. A body that cannot be published is refused and nothing is created. In a team project the
  /// person must be an admin, and the version waits for the server.
  pub fn create_published<B: ResourceBody + Clone>(
    &mut self,
    slug: Slug,
    name: Name,
    body: &B,
    now: Timestamp,
  ) -> Result<(Resource, Version<B>), StoreError> {
    let transaction = self.connection.transaction()?;
    let resource = insert::<B>(&transaction, slug, name, now)?;
    let version = publish_prepared(
      &transaction,
      resource.id(),
      Draft::new(body.clone(), None, now),
      None,
      now,
    )?;
    // A project that is not shared waits for no server, so no row says it.
    transaction.execute(
      "UPDATE shared_publishes SET created = 1 WHERE version_id = ?1",
      [version.id().to_string()],
    )?;
    transaction.commit()?;
    Ok((resource, version))
  }

  /// Takes back the resource `resource` that [`ProjectDb::create_published`] made, whose only version the server has
  /// not numbered, e.g. because it refused it: the resource and its version are deleted, as if never created. A
  /// resource with a version the server numbered is left ([`StoreError::AlreadyNumbered`]): it is the team's now.
  pub fn discard_created(&mut self, resource: ResourceId) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    // The versions go before their rows in `shared_publishes`, which the delete trigger reads.
    transaction.pragma_update(None, "defer_foreign_keys", true)?;
    let confirmed: bool = transaction.query_row(
      "SELECT EXISTS (SELECT 1 FROM resource_versions WHERE resource_id = ?1 \
       AND id NOT IN (SELECT version_id FROM shared_publishes))",
      [resource.to_string()],
      |row| row.get(0),
    )?;
    if confirmed {
      return Err(StoreError::AlreadyNumbered(resource));
    }
    let id = resource.to_string();
    transaction.execute("DELETE FROM resource_versions WHERE resource_id = ?1", [&id])?;
    transaction.execute("DELETE FROM shared_publishes WHERE resource_id = ?1", [&id])?;
    transaction.execute("DELETE FROM resource_drafts WHERE resource_id = ?1", [&id])?;
    transaction.execute("DELETE FROM resources WHERE id = ?1", [&id])?;
    transaction.commit()?;
    Ok(())
  }
}
