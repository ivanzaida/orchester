//! Removing a resource and discarding its draft (ORC-107). Both decide from what the write transaction reads, so a
//! save or a pipeline edit made meanwhile is never lost or undone.

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, Removal, ResourceBody, ResourceKind, StepUser, Version, VersionNumber, pipeline_step_users},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{
  ProjectDb,
  resources::{check_kind, kind_text, load_draft, load_resources},
  used_versions::used_here,
};
use crate::StoreError;

const LIVE: &str = "id NOT IN (SELECT resource_id FROM removed_resources)";

impl ProjectDb {
  /// Deletes the draft of `resource` while it is still `expected`, so the latest published version is the resource
  /// again; that version's number. A resource never published is removed instead ([`StoreError::NotPublished`]).
  pub fn discard_draft_matching<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    expected: &B,
  ) -> Result<VersionNumber, StoreError> {
    let transaction = self
      .connection
      .transaction_with_behavior(TransactionBehavior::Immediate)?;
    check_kind::<B>(&transaction, resource)?;
    let draft = load_draft::<B>(&transaction, resource)?.ok_or(StoreError::NoDraft(resource))?;
    if draft.body() != expected {
      return Err(StoreError::DraftChanged(resource));
    }
    let latest = latest::<B>(&transaction, resource)?.ok_or(StoreError::NotPublished(resource))?;
    transaction.execute(
      "DELETE FROM resource_drafts WHERE resource_id = ?1",
      [resource.to_string()],
    )?;
    transaction.commit()?;
    Ok(latest.number())
  }

  /// When resource `id` was removed; `None` while it is not.
  pub fn resource_removed_at(&self, id: ResourceId) -> Result<Option<Timestamp>, StoreError> {
    removed_at(&self.connection, id)
  }

  /// Follows the server's archived mark of the team's resource `id`: archived, it is removed here as of `now`, so no
  /// list shows it and no new run pins it; restored, it is back. A team resource cannot be removed here otherwise
  /// ([`super::shared_publishes::ensure_may_change`] refuses), so restoring undoes only the server's archive. Whether
  /// anything changed; a resource this computer does not keep as the team's (a skill, or one kept *Not shared*) changes
  /// nothing.
  pub fn mark_shared_archived(&mut self, id: ResourceId, archived: bool, now: Timestamp) -> Result<bool, StoreError> {
    set_archived(&self.connection, id, archived, now)
  }
}

fn set_archived(connection: &Connection, id: ResourceId, archived: bool, now: Timestamp) -> Result<bool, StoreError> {
  let changed = if archived {
    connection.execute(
      "INSERT INTO removed_resources (resource_id, removed_at) SELECT id, ?2 FROM resources \
       WHERE id = ?1 AND sharing = 'shared' ON CONFLICT (resource_id) DO NOTHING",
      params![id.to_string(), now.unix_millis()],
    )?
  } else {
    connection.execute(
      "DELETE FROM removed_resources WHERE resource_id = ?1 \
       AND resource_id IN (SELECT id FROM resources WHERE sharing = 'shared')",
      [id.to_string()],
    )?
  };
  Ok(changed > 0)
}

pub(super) fn remove(connection: &mut Connection, id: ResourceId, now: Timestamp) -> Result<Removal, StoreError> {
  let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
  let resource = load_resources(&transaction, "WHERE id = ?1", [id.to_string()])?
    .pop()
    .ok_or(StoreError::ResourceNotFound(id))?;
  super::shared_publishes::ensure_may_change(&transaction, id)?;
  if removed_at(&transaction, id)?.is_some() {
    return Err(StoreError::ResourceNotFound(id));
  }
  // Only profiles and tools are named by pipeline steps; instructions and skills reach agents by their scope.
  if matches!(resource.kind(), ResourceKind::AgentProfile | ResourceKind::Tool) {
    let users = step_users(&transaction, id)?;
    if !users.is_empty() {
      // Dropping the transaction rolls it back: nothing changed, and the steps that run it now are named.
      return Ok(Removal::InUse(users));
    }
  }
  transaction.execute(
    "INSERT INTO removed_resources (resource_id, removed_at) VALUES (?1, ?2)",
    params![id.to_string(), now.unix_millis()],
  )?;
  transaction.commit()?;
  Ok(Removal::Removed)
}

/// The steps of the latest versions of the project's live pipelines that run `executor`.
fn step_users(connection: &Connection, executor: ResourceId) -> Result<Vec<StepUser>, StoreError> {
  let pipelines = load_resources(
    connection,
    &format!("WHERE kind = ?1 AND {LIVE} ORDER BY slug"),
    [kind_text(ResourceKind::Pipeline)],
  )?;
  let mut users = Vec::new();
  for pipeline in pipelines {
    let latest = latest::<Pipeline>(connection, pipeline.id())?;
    users.extend(pipeline_step_users(
      (pipeline.id(), pipeline.name()),
      latest.as_ref(),
      executor,
    ));
  }
  Ok(users)
}

fn latest<B: ResourceBody>(connection: &Connection, resource: ResourceId) -> Result<Option<Version<B>>, StoreError> {
  used_here(connection, resource)
}

pub(super) fn removed_at(connection: &Connection, id: ResourceId) -> Result<Option<Timestamp>, StoreError> {
  let at: Option<i64> = connection
    .query_row(
      "SELECT removed_at FROM removed_resources WHERE resource_id = ?1",
      [id.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  Ok(at.map(Timestamp::from_unix_millis))
}
