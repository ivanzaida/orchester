//! Task and project memory in the project database: `memories` with its two full-text indexes,
//! `memory_settings` (Project settings › Memory) and `invocation_memories`, what each invocation's
//! prompt was given. A record's key names one active record of its scope; deleting a record deletes
//! the earlier versions it superseded with it. The bundled SQLite is built with FTS5 (rusqlite's `bundled` feature
//! compiles it with `SQLITE_ENABLE_FTS5`), and its trigram tokenizer exists since SQLite 3.34.

mod rows;
mod search;

use orchester_core::{
  id::{InvocationId, MemoryId, TaskId},
  memory::{
    InjectedMemory, InjectionReason, Memory, MemoryFilter, MemoryLimits, MemoryMatch, MemoryQuery, MemorySettings,
    MemoryStatus, MemoryStore, ScopeFilter, WritePolicy,
  },
};
use rusqlite::{OptionalExtension, params};

use self::rows::{COLUMNS, Raw};
use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl MemoryStore for ProjectDb {
  type Error = StoreError;

  fn add_memories(&mut self, memories: &[Memory]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    for memory in memories {
      rows::upsert(&transaction, memory)?;
    }
    transaction.commit()?;
    Ok(())
  }

  fn save_memory(&mut self, memory: &Memory) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if rows::status(&transaction, memory.id())?.is_none() {
      return Err(StoreError::MemoryNotFound(memory.id()));
    }
    rows::upsert(&transaction, memory)?;
    transaction.commit()?;
    Ok(())
  }

  fn supersede_memory(&mut self, superseded: &Memory, replacement: &Memory) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    match rows::status(&transaction, superseded.id())? {
      Some(MemoryStatus::Active) => {}
      Some(_) => return Err(StoreError::MemoryNotActive(superseded.id())),
      None => return Err(StoreError::MemoryNotFound(superseded.id())),
    }
    rows::upsert(&transaction, superseded)?;
    rows::upsert(&transaction, replacement)?;
    transaction.commit()?;
    Ok(())
  }

  fn delete_memory(&mut self, id: MemoryId) -> Result<(), StoreError> {
    let deleted = self.connection.execute(
      "WITH RECURSIVE history (id) AS (SELECT ?1 UNION SELECT m.supersedes_id FROM memories m        JOIN history h ON m.id = h.id WHERE m.supersedes_id IS NOT NULL)        DELETE FROM memories WHERE id IN (SELECT id FROM history)",
      [id.to_string()],
    )?;
    if deleted == 0 {
      return Err(StoreError::MemoryNotFound(id));
    }
    Ok(())
  }

  fn memory(&self, id: MemoryId) -> Result<Option<Memory>, StoreError> {
    rows::load(&self.connection, id)
  }

  fn memories(&self, filter: &MemoryFilter) -> Result<Vec<Memory>, StoreError> {
    let (scope, task) = match filter.scope {
      ScopeFilter::Task(task) => ("task", Some(task)),
      ScopeFilter::Project => ("project", None),
      ScopeFilter::TaskAndProject(task) => ("both", Some(task)),
    };
    let sql = format!(
      "SELECT {COLUMNS} FROM memories m WHERE \
       CASE ?1 WHEN 'task' THEN m.task_id = ?2 WHEN 'project' THEN m.task_id IS NULL \
       ELSE m.task_id = ?2 OR m.task_id IS NULL END \
       AND (?3 IS NULL OR m.status = ?3) AND (?4 = 0 OR m.pinned = 1) ORDER BY m.created_at, m.seq"
    );
    let mut statement = self.connection.prepare(&sql)?;
    let rows = statement.query_map(
      params![
        scope,
        task.map(|task| task.to_string()),
        filter.status.map(MemoryStatus::name),
        filter.pinned_only,
      ],
      Raw::from_row,
    )?;
    rows.map(|row| row?.into_memory()).collect()
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
      "INSERT INTO memory_settings (id, task_writes, project_writes, max_records, max_tokens) \
       VALUES (1, ?1, ?2, ?3, ?4) ON CONFLICT (id) DO UPDATE SET task_writes = excluded.task_writes, \
       project_writes = excluded.project_writes, max_records = excluded.max_records, \
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
    let transaction = self.connection.transaction()?;
    for (position, entry) in injected.iter().enumerate() {
      let changed = transaction.execute(
        "INSERT INTO invocation_memories (invocation_id, position, memory_id, reason, tokens) \
         SELECT ?1, ?2, ?3, ?4, ?5 WHERE EXISTS (SELECT 1 FROM invocations WHERE id = ?1)",
        params![
          invocation.to_string(),
          i64::try_from(position).map_err(|_| StoreError::Unrepresentable("memory position"))?,
          entry.memory.to_string(),
          entry.reason.name(),
          entry.tokens,
        ],
      )?;
      if changed == 0 {
        return Err(StoreError::InvocationNotFound(invocation));
      }
    }
    transaction.commit()?;
    Ok(())
  }

  fn injected_memory(&self, invocation: InvocationId) -> Result<Vec<InjectedMemory>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT memory_id, reason, tokens FROM invocation_memories WHERE invocation_id = ?1 ORDER BY position",
    )?;
    let rows = statement.query_map([invocation.to_string()], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, u32>(2)?,
      ))
    })?;
    rows
      .map(|row| {
        let (memory, reason, tokens) = row?;
        Ok(InjectedMemory {
          memory: crate::rows::parse("injected memory id", &memory)?,
          reason: InjectionReason::from_name(&reason).ok_or_else(|| corrupt("injection reason", &reason))?,
          tokens,
        })
      })
      .collect()
  }
}
