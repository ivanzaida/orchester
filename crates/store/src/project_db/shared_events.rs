//! What a shared project's event stream writes to the cache (`docs/architecture/server.md`, "Cursor and applying
//! events"): a snapshot replaces the confirmed cache and sets the cursor, and each delta's writes go in one transaction
//! with the cursor it advances to, so a crash replays at most the delta it was applying.

use orchester_core::{
  id::{TagId, TaskId},
  sharing::MemberRole,
  task::organization::Tag,
  time::Timestamp,
};
use rusqlite::{Connection, Transaction, params};

use super::{
  ProjectDb, shared_modules,
  shared_tasks::{self, CachedTask},
  sharing::{self, role_text},
  tasks,
};
use crate::{
  StoreError,
  rows::{corrupt, parse, tokens_column},
};

/// The snapshot a stream connected without a cursor gets: the project as the connected member reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedSnapshot {
  pub role: Option<MemberRole>,
  pub tags: Vec<Tag>,
  pub modules: Vec<shared_modules::CachedModule>,
  pub tasks: Vec<CachedTask>,
  /// The sequence of the last event the snapshot includes.
  pub cursor: u64,
}

/// What one delta changes in the cache; empty for a kind the cache does not keep.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SharedEvent {
  pub tasks: Vec<CachedTask>,
  /// Tags created or renamed, written by id.
  pub tags: Vec<Tag>,
  pub removed_tags: Vec<TagId>,
  pub modules: Vec<shared_modules::CachedModule>,
}

impl ProjectDb {
  /// The shared task `id` as the cache has it; `None` when the cache does not hold it.
  pub fn cached_task(&self, id: TaskId) -> Result<Option<CachedTask>, StoreError> {
    cached(&self.connection, id)
  }

  /// Every shared task as the cache has it.
  pub fn cached_tasks(&self) -> Result<Vec<CachedTask>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT id FROM tasks WHERE sharing = 'shared' ORDER BY number")?;
    let ids = statement
      .query_map([], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    ids
      .iter()
      .filter_map(|id| {
        parse("task id", id)
          .and_then(|id| cached(&self.connection, id))
          .transpose()
      })
      .collect()
  }

  /// Replaces the confirmed cache with `snapshot` and continues the stream from its cursor, in one transaction; the
  /// cache matches the server as of `at`, and the person is a member again.
  pub fn take_shared_snapshot(&mut self, snapshot: &SharedSnapshot, at: Timestamp) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if sharing::link(&transaction)?.is_none() {
      return Err(StoreError::NotShared);
    }
    shared_tasks::replace_tags(&transaction, &snapshot.tags)?;
    shared_modules::write(&transaction, &snapshot.modules)?;
    for task in &snapshot.tasks {
      shared_tasks::write_cached(&transaction, task, None)?;
    }
    transaction.execute(
      "UPDATE sharing SET role = ?1, event_cursor = ?2, last_synced_at = ?3, removed_at = NULL",
      params![
        snapshot.role.map(role_text),
        tokens_column("event cursor", snapshot.cursor)?,
        at.unix_millis()
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// Writes `event` and advances the cursor to `cursor`, in one transaction; the cache matches the server as of `at`.
  /// The cursor never moves back.
  pub fn take_shared_event(&mut self, event: &SharedEvent, cursor: u64, at: Timestamp) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if sharing::link(&transaction)?.is_none() {
      return Err(StoreError::NotShared);
    }
    write_tags(&transaction, &event.tags)?;
    for tag in &event.removed_tags {
      transaction.execute("DELETE FROM project_tags WHERE id = ?1", [tag.to_string()])?;
    }
    shared_modules::write(&transaction, &event.modules)?;
    for task in &event.tasks {
      shared_tasks::write_cached(&transaction, task, None)?;
    }
    transaction.execute(
      "UPDATE sharing SET event_cursor = MAX(event_cursor, ?1), last_synced_at = ?2",
      params![tokens_column("event cursor", cursor)?, at.unix_millis()],
    )?;
    transaction.commit()?;
    Ok(())
  }
}

/// Writes `tags` by id: a new tag is added, a known one takes its name.
fn write_tags(transaction: &Transaction<'_>, tags: &[Tag]) -> Result<(), StoreError> {
  for tag in tags {
    let name = tag.name().as_str();
    transaction.execute(
      "INSERT INTO project_tags (id, name, normalized) VALUES (?1, ?2, ?3) \
       ON CONFLICT (id) DO UPDATE SET name = excluded.name, normalized = excluded.normalized",
      params![tag.id().to_string(), name, name.to_lowercase()],
    )?;
  }
  Ok(())
}

fn cached(connection: &Connection, id: TaskId) -> Result<Option<CachedTask>, StoreError> {
  let project = shared_tasks::project_key(connection)?;
  let Some(task) = tasks::load_tasks(
    connection,
    &project,
    "WHERE id = ?1 AND sharing = 'shared'",
    [id.to_string()],
  )?
  .pop() else {
    return Ok(None);
  };
  let Some(info) = shared_tasks::info(connection, id)? else {
    return Ok(None);
  };
  let details = task.details();
  // A shared row is written only by `write_cached`, which sets who created and changed it and when.
  let (created_by, updated_by, updated_at) = match (info.created_by, info.updated_by, info.updated_at) {
    (Some(created_by), Some(updated_by), Some(updated_at)) => (created_by, updated_by, updated_at),
    _ => return Err(corrupt("shared task attribution", id)),
  };
  Ok(Some(CachedTask {
    id,
    number: task.key().number(),
    revision: task.revision(),
    title: details.title.clone(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: info.pipeline_slug,
    limits: details.limits,
    status: task.status(),
    organization: task.organization().clone(),
    module: task.module(),
    former_key: info.former_key,
    created_by,
    created_at: task.created_at(),
    updated_by,
    updated_at,
    claim: info.claim,
    run: info.run,
  }))
}
