//! Removing a resource and discarding its draft (ORC-107). Both decide from what the write transaction reads, so a
//! save or a pipeline edit made meanwhile is never lost or undone.

use orchester_core::{
  id::ResourceId,
  resource::{
    Draft, Pipeline, Removal, RemoveMode, ResourceBody, ResourceKind, Version, VersionNumber, pipeline_step_users,
  },
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{
  ProjectDb,
  resources::{check_kind, kind_text, load_draft, load_resources, save_draft},
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
}

pub(super) fn remove(
  connection: &mut Connection,
  id: ResourceId,
  mode: &RemoveMode,
  now: Timestamp,
) -> Result<Removal, StoreError> {
  let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
  let resource = load_resources(&transaction, "WHERE id = ?1", [id.to_string()])?
    .pop()
    .ok_or(StoreError::ResourceNotFound(id))?;
  super::shared_publishes::ensure_may_change(&transaction, id)?;
  if removed_at(&transaction, id)?.is_some() {
    return Err(StoreError::ResourceNotFound(id));
  }
  let mut users = Vec::new();
  let mut running = Vec::new();
  // Only profiles and tools are named by pipeline steps; instructions and skills reach agents by their scope.
  if matches!(resource.kind(), ResourceKind::AgentProfile | ResourceKind::Tool) {
    let pipelines = load_resources(
      &transaction,
      &format!("WHERE kind = ?1 AND {LIVE} ORDER BY slug"),
      [kind_text(ResourceKind::Pipeline)],
    )?;
    for pipeline in pipelines {
      let draft = load_draft::<Pipeline>(&transaction, pipeline.id())?;
      let latest = latest::<Pipeline>(&transaction, pipeline.id())?;
      let found = pipeline_step_users((pipeline.id(), pipeline.name()), (draft.as_ref(), latest.as_ref()), id);
      if !found.is_empty() {
        users.extend(found);
        running.push((pipeline.id(), draft, latest));
      }
    }
  }
  if !mode.may_go_on(&users) {
    // Dropping the transaction rolls it back: nothing changed, and the steps that run it now are asked about.
    return Ok(Removal::InUse(users));
  }
  for (pipeline, draft, latest) in running {
    reset(&transaction, pipeline, (draft, latest), (id, now))?;
  }
  transaction.execute(
    "INSERT INTO removed_resources (resource_id, removed_at) VALUES (?1, ?2)",
    params![id.to_string(), now.unix_millis()],
  )?;
  transaction.commit()?;
  Ok(Removal::Removed)
}

/// Saves the pipeline's draft (started from its latest version when it has none) without `executor`.
fn reset(
  connection: &Connection,
  pipeline: ResourceId,
  (draft, latest): (Option<Draft<Pipeline>>, Option<Version<Pipeline>>),
  (executor, now): (ResourceId, Timestamp),
) -> Result<(), StoreError> {
  let (body, based_on) = match (draft, latest) {
    (Some(draft), _) => (draft.body().clone(), draft.based_on()),
    (None, Some(latest)) => (latest.body().clone(), Some(latest.number())),
    (None, None) => return Ok(()),
  };
  let cleared = body.without_executor(executor);
  if cleared == body {
    return Ok(());
  }
  save_draft(connection, pipeline, &Draft::new(cleared, based_on, now)).map(|_| ())
}

fn latest<B: ResourceBody>(connection: &Connection, resource: ResourceId) -> Result<Option<Version<B>>, StoreError> {
  used_here(connection, resource)
}

fn removed_at(connection: &Connection, id: ResourceId) -> Result<Option<Timestamp>, StoreError> {
  let at: Option<i64> = connection
    .query_row(
      "SELECT removed_at FROM removed_resources WHERE resource_id = ?1",
      [id.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  Ok(at.map(Timestamp::from_unix_millis))
}
