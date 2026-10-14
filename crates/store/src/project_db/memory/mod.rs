//! Task and project memory in the project database: `memories` with its two full-text indexes,
//! `memory_settings` (Project settings › Memory) and `invocation_memories`, what each invocation's
//! prompt was given. A record's key names one active record of its scope; deleting a record deletes
//! the earlier versions it superseded with it. The bundled SQLite is built with FTS5 (rusqlite's `bundled` feature
//! compiles it with `SQLITE_ENABLE_FTS5`), and its trigram tokenizer exists since SQLite 3.34.

mod injected;
mod rows;
mod search;

use orchester_core::{
  id::{InvocationId, MemoryId, TaskId},
  memory::{
    InjectedMemory, Memory, MemoryChanges, MemoryFilter, MemoryLimits, MemoryMatch, MemoryQuery, MemorySettings,
    MemoryStatus, MemoryStore, ScopeFilter, UnreadableMemory, WritePolicy,
  },
};
use rusqlite::{Connection, OptionalExtension, params};

use self::rows::COLUMNS;
use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

/// Deletes record `?1` and its history: its earlier versions ([`Memory::earlier_version`]), and
/// theirs. A record names an earlier version only once it was active (it is active, superseded or
/// deprecated) and that version is superseded. A proposal, a rejected or a withdrawn record names
/// only what it would have replaced; deleting it leaves that record alone.
const DELETE_WITH_HISTORY: &str = "
  WITH RECURSIVE history (id) AS (
    SELECT id FROM memories WHERE id = ?1
    UNION
    SELECT older.id FROM history
      JOIN memories newer ON newer.id = history.id
      JOIN memories older ON older.id = newer.supersedes_id
      WHERE older.status = 'superseded' AND newer.status IN ('active', 'superseded', 'deprecated')
  )
  DELETE FROM memories WHERE id IN (SELECT id FROM history)";

/// The readable records `filter` selects, oldest first.
fn list(connection: &Connection, filter: &MemoryFilter) -> Result<Vec<Memory>, StoreError> {
  let (scope, task) = match filter.scope {
    ScopeFilter::Task(task) => ("task", Some(task)),
    ScopeFilter::Project => ("project", None),
    ScopeFilter::TaskAndProject(task) => ("both", Some(task)),
  };
  let sql = format!(
    "
    SELECT {COLUMNS} FROM memories m
    WHERE CASE ?1
        WHEN 'task' THEN m.task_id = ?2
        WHEN 'project' THEN m.task_id IS NULL
        ELSE m.task_id = ?2 OR m.task_id IS NULL
      END
      AND (?3 IS NULL OR m.status = ?3)
      AND (?4 = 0 OR m.pinned = 1)
    ORDER BY m.created_at, m.seq"
  );
  let mut statement = connection.prepare(&sql)?;
  let rows = statement.query_map(
    params![
      scope,
      task.map(|task| task.to_string()),
      filter.status.map(MemoryStatus::name),
      filter.pinned_only,
    ],
    rows::read,
  )?;
  readable(rows)
}

/// Stores `changes`: the changed records first, under the revision rule, then the added ones.
fn commit(connection: &Connection, changes: &MemoryChanges) -> Result<(), StoreError> {
  for memory in &changes.changed {
    rows::update(connection, memory)?;
  }
  for memory in &changes.added {
    rows::insert(connection, memory)?;
  }
  Ok(())
}

/// The records of `rows` that can be read back; the others are left out, and
/// [`MemoryStore::unreadable_memories`] lists them.
fn readable(
  rows: impl Iterator<Item = rusqlite::Result<(String, Result<Memory, String>)>>,
) -> Result<Vec<Memory>, StoreError> {
  let mut memories = Vec::new();
  for row in rows {
    if let (_, Ok(memory)) = row? {
      memories.push(memory);
    }
  }
  Ok(memories)
}

impl ProjectDb {
  /// The readable records with `status`, of the project and of every task, oldest first, in one query.
  pub fn memories_with_status(&self, status: MemoryStatus) -> Result<Vec<Memory>, StoreError> {
    let sql = format!("SELECT {COLUMNS} FROM memories m WHERE m.status = ?1 ORDER BY m.created_at, m.seq");
    let mut statement = self.connection.prepare(&sql)?;
    let rows = statement.query_map([status.name()], rows::read)?;
    readable(rows)
  }
}

impl MemoryStore for ProjectDb {
  type Error = StoreError;

  fn add_memories(&mut self, memories: &[Memory]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    for memory in memories {
      rows::insert(&transaction, memory)?;
    }
    transaction.commit()?;
    Ok(())
  }

  fn commit_memories(&mut self, changes: &mut MemoryChanges) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    commit(&transaction, changes)?;
    transaction.commit()?;
    for memory in &mut changes.changed {
      memory.saved();
    }
    Ok(())
  }

  fn change_memories<E>(
    &mut self,
    filter: &MemoryFilter,
    change: impl FnOnce(Vec<Memory>) -> Result<MemoryChanges, E>,
  ) -> Result<Result<(), E>, StoreError> {
    // Immediate (the connection's behavior): the write lock is held from the read on.
    let transaction = self.connection.transaction()?;
    let memories = list(&transaction, filter)?;
    let changes = match change(memories) {
      Ok(changes) => changes,
      Err(error) => return Ok(Err(error)),
    };
    commit(&transaction, &changes)?;
    transaction.commit()?;
    Ok(Ok(()))
  }

  fn save_memory(&mut self, memory: &mut Memory) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    rows::update(&transaction, memory)?;
    transaction.commit()?;
    memory.saved();
    Ok(())
  }

  fn supersede_memory(&mut self, superseded: &mut Memory, replacement: &mut Memory) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    match rows::status(&transaction, superseded.id())? {
      Some(MemoryStatus::Active) => {}
      Some(_) => return Err(StoreError::MemoryNotActive(superseded.id())),
      None => return Err(StoreError::MemoryNotFound(superseded.id())),
    }
    rows::update(&transaction, superseded)?;
    // A new replacement, or a proposal that was just approved.
    let approved = match rows::status(&transaction, replacement.id())? {
      None => {
        rows::insert(&transaction, replacement)?;
        false
      }
      Some(MemoryStatus::Proposed) => {
        rows::update(&transaction, replacement)?;
        true
      }
      Some(_) => return Err(StoreError::MemoryNotActive(replacement.id())),
    };
    transaction.commit()?;
    superseded.saved();
    if approved {
      replacement.saved();
    }
    Ok(())
  }

  fn delete_memory(&mut self, id: MemoryId) -> Result<(), StoreError> {
    let deleted = self.connection.execute(DELETE_WITH_HISTORY, [id.to_string()])?;
    if deleted == 0 {
      return Err(StoreError::MemoryNotFound(id));
    }
    Ok(())
  }

  fn memory(&self, id: MemoryId) -> Result<Option<Memory>, StoreError> {
    rows::load(&self.connection, id)
  }

  fn memories(&self, filter: &MemoryFilter) -> Result<Vec<Memory>, StoreError> {
    list(&self.connection, filter)
  }

  fn unreadable_memories(&self) -> Result<Vec<UnreadableMemory>, StoreError> {
    let sql = format!("SELECT {COLUMNS} FROM memories m ORDER BY m.seq");
    let mut statement = self.connection.prepare(&sql)?;
    let rows = statement.query_map([], rows::read)?;
    let mut unreadable = Vec::new();
    for row in rows {
      if let (id, Err(detail)) = row? {
        unreadable.push(UnreadableMemory { id, detail });
      }
    }
    Ok(unreadable)
  }

  fn search_memories(&self, task: TaskId, query: &MemoryQuery, limit: usize) -> Result<Vec<MemoryMatch>, StoreError> {
    search::search(&self.connection, task, query, limit)
  }

  fn memory_settings(&self) -> Result<MemorySettings, StoreError> {
    let row: Option<(String, String, i64, i64)> = self
      .connection
      .query_row(
        "SELECT task_writes, project_writes, max_records, max_tokens FROM memory_settings",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
      )
      .optional()?;
    let Some((task_writes, project_writes, records, tokens)) = row else {
      return Ok(MemorySettings::DEFAULT);
    };
    let policy = |name: &str| WritePolicy::from_name(name).ok_or_else(|| corrupt("memory write policy", name));
    let records = u16::try_from(records).map_err(|_| corrupt("memory record limit", records))?;
    let tokens = u32::try_from(tokens).map_err(|_| corrupt("memory token limit", tokens))?;
    Ok(MemorySettings {
      task_writes: policy(&task_writes)?,
      project_writes: policy(&project_writes)?,
      limits: MemoryLimits::new(records, tokens).map_err(|error| corrupt("memory limits", error))?,
    })
  }

  fn save_memory_settings(&mut self, settings: &MemorySettings) -> Result<(), StoreError> {
    self.connection.execute(
      "
      INSERT INTO memory_settings (id, task_writes, project_writes, max_records, max_tokens)
      VALUES (1, ?1, ?2, ?3, ?4)
      ON CONFLICT (id) DO UPDATE SET
        task_writes = excluded.task_writes,
        project_writes = excluded.project_writes,
        max_records = excluded.max_records,
        max_tokens = excluded.max_tokens",
      params![
        settings.task_writes.name(),
        settings.project_writes.name(),
        settings.limits.records(),
        settings.limits.tokens(),
      ],
    )?;
    Ok(())
  }

  fn save_injected_memory(&mut self, invocation: InvocationId, injected: &[InjectedMemory]) -> Result<(), StoreError> {
    injected::save(&mut self.connection, invocation, injected)
  }

  fn injected_memory(&self, invocation: InvocationId) -> Result<Vec<InjectedMemory>, StoreError> {
    injected::load(&self.connection, invocation)
  }
}
