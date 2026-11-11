//! Removing a resource and discarding its draft (ORC-107). Both decide from what the write transaction reads, so a
//! save or a pipeline edit made meanwhile is never lost or undone.

use orchester_core::{
  id::ResourceId,
  resource::{
    Draft, Pipeline, Removal, Resource, ResourceBody, ResourceKind, StepUser, Version, VersionNumber,
    pipeline_step_users,
  },
  sharing::TaskRevision,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{
  ProjectDb,
  resources::{check_kind, kind_text, load_draft, load_resources, save_draft},
  shared_versions::{ResourceSharing, set_sharing},
  used_versions::used_here,
};
use crate::StoreError;

/// What [`ProjectDb::settle_draft`] does with a resource's draft.
pub enum DraftChange<B> {
  /// The draft stays as it is.
  Keep,
  /// This draft is kept instead.
  Replace(Draft<B>),
  /// The draft is thrown away; the version in use is the resource again.
  Discard,
}

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

  /// Settles the draft of `resource` from the draft and the version this computer uses, both read in the write
  /// transaction that applies the answer, so a save and a discard that race never act on what the other just changed.
  /// A resource never published is [`StoreError::NotPublished`].
  pub fn settle_draft<B: ResourceBody>(
    &mut self,
    resource: ResourceId,
    decide: impl FnOnce(Option<&Draft<B>>, &Version<B>) -> Result<DraftChange<B>, StoreError>,
  ) -> Result<(), StoreError> {
    let transaction = self
      .connection
      .transaction_with_behavior(TransactionBehavior::Immediate)?;
    check_kind::<B>(&transaction, resource)?;
    let draft = load_draft::<B>(&transaction, resource)?;
    let latest = used_here::<B>(&transaction, resource)?.ok_or(StoreError::NotPublished(resource))?;
    match decide(draft.as_ref(), &latest)? {
      DraftChange::Keep => return Ok(()),
      DraftChange::Replace(replacement) => {
        save_draft(&transaction, resource, &replacement)?;
      }
      DraftChange::Discard => {
        transaction.execute(
          "DELETE FROM resource_drafts WHERE resource_id = ?1",
          [resource.to_string()],
        )?;
      }
    }
    transaction.commit()?;
    Ok(())
  }

  /// When resource `id` was removed; `None` while it is not.
  pub fn resource_removed_at(&self, id: ResourceId) -> Result<Option<Timestamp>, StoreError> {
    removed_at(&self.connection, id)
  }

  /// The team's resources of `kind` that the server archived and that still keep a draft of this computer's, each with
  /// when it was removed here: the draft stays to copy, discard, or publish once the resource is restored.
  pub fn archived_with_draft(&self, kind: ResourceKind) -> Result<Vec<(Resource, Timestamp)>, StoreError> {
    let archived = load_resources(
      &self.connection,
      "WHERE kind = ?1 AND sharing = 'shared' AND id IN (SELECT resource_id FROM removed_resources) \
       AND id IN (SELECT resource_id FROM resource_drafts) ORDER BY slug",
      [kind_text(kind)],
    )?;
    archived
      .into_iter()
      .map(|resource| {
        let at = removed_at(&self.connection, resource.id())?.ok_or(StoreError::ResourceNotFound(resource.id()))?;
        Ok((resource, at))
      })
      .collect()
  }

  /// Follows the server's archived mark of the team's resource `id`: archived, it is removed here as of `now`, so no
  /// list shows it and no new run pins it; restored, it is back. A team resource cannot be removed here otherwise
  /// (`RemoteResourceStore` refuses), so restoring undoes only the server's archive. Whether anything changed; a
  /// resource this computer does not keep as the team's (a skill, or one kept *Not shared*) changes nothing.
  pub fn mark_shared_archived(&mut self, id: ResourceId, archived: bool, now: Timestamp) -> Result<bool, StoreError> {
    set_archived(&self.connection, id, archived, now)
  }

  /// As [`ProjectDb::mark_shared_archived`], and keeps the revision the server gave the resource with it, in one
  /// transaction: a failure in between leaves neither the mark nor the revision changed.
  pub fn mark_shared_archived_at(
    &mut self,
    id: ResourceId,
    archived: bool,
    revision: TaskRevision,
    now: Timestamp,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let changed = set_archived(&transaction, id, archived, now)?;
    set_sharing(&transaction, id, Some(ResourceSharing::Shared(revision)))?;
    transaction.commit()?;
    Ok(changed)
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
