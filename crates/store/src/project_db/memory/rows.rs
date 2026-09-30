//! A memory record as a row of `memories`, both ways. Reads go back through the domain
//! constructors, so a damaged row is reported as corrupt instead of read as a record.

use orchester_core::{
  id::MemoryId,
  memory::{
    AgentProvenance, Confidence, Importance, Memory, MemoryAuthor, MemoryContent, MemoryKey, MemoryKind, MemoryScope,
    MemoryStatus, ScreenedContent, StoredMemory, Tags,
  },
  resource::StepKey,
  time::Timestamp,
};
use rusqlite::{Connection, Row, params};

use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The columns [`read`] expects, of `memories` as `m`.
pub(super) const COLUMNS: &str = "m.id, m.task_id, m.kind, m.key, m.text, m.tags, m.importance, m.confidence, \
                                  m.status, m.pinned, m.supersedes_id, m.author, m.profile_id, m.run_id, m.step_key, \
                                  m.step_execution_id, m.source_events, m.created_at, m.updated_at";
/// How many of the first columns of a query [`COLUMNS`] are.
pub(super) const COLUMN_COUNT: usize = 19;

/// A row as stored, before it is checked.
pub(super) struct Raw {
  id: String,
  task: Option<String>,
  kind: String,
  key: Option<String>,
  text: String,
  tags: String,
  importance: i64,
  confidence: i64,
  status: String,
  pinned: bool,
  supersedes: Option<String>,
  author: String,
  agent: [Option<String>; 4],
  sources: String,
  created_at: i64,
  updated_at: i64,
}

impl Raw {
  pub(super) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      task: row.get(1)?,
      kind: row.get(2)?,
      key: row.get(3)?,
      text: row.get(4)?,
      tags: row.get(5)?,
      importance: row.get(6)?,
      confidence: row.get(7)?,
      status: row.get(8)?,
      pinned: row.get(9)?,
      supersedes: row.get(10)?,
      author: row.get(11)?,
      agent: [row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?],
      sources: row.get(16)?,
      created_at: row.get(17)?,
      updated_at: row.get(18)?,
    })
  }

  pub(super) fn into_memory(self) -> Result<Memory, StoreError> {
    const WHAT: &str = "memory record";
    let bad = |detail: &str| corrupt(WHAT, detail);
    let kind = MemoryKind::from_name(&self.kind).ok_or_else(|| bad("unknown kind"))?;
    let importance = u8::try_from(self.importance)
      .map_err(|_| bad("importance out of range"))
      .and_then(|value| Importance::new(value).map_err(|error| corrupt(WHAT, error)))?;
    let confidence = u8::try_from(self.confidence)
      .map_err(|_| bad("confidence out of range"))
      .and_then(|value| Confidence::new(value).map_err(|error| corrupt(WHAT, error)))?;
    let mut content = MemoryContent::new(kind, &self.text)
      .map_err(|error| corrupt(WHAT, error))?
      .with_tags(Tags::new(self.tags.split_whitespace()).map_err(|error| corrupt(WHAT, error))?)
      .with_importance(importance)
      .with_confidence(confidence);
    if let Some(key) = self.key {
      content = content.with_key(MemoryKey::new(&key).map_err(|error| corrupt(WHAT, error))?);
    }
    Ok(Memory::from_stored(StoredMemory {
      id: parse("memory id", &self.id)?,
      scope: match self.task {
        Some(task) => MemoryScope::Task(parse("memory task", &task)?),
        None => MemoryScope::Project,
      },
      author: author(&self.author, self.agent)?,
      content: ScreenedContent::from_stored(content),
      status: MemoryStatus::from_name(&self.status).ok_or_else(|| bad("unknown status"))?,
      pinned: self.pinned,
      supersedes: self
        .supersedes
        .map(|id| parse("superseded memory id", &id))
        .transpose()?,
      sources: serde_json::from_str(&self.sources).map_err(|error| corrupt(WHAT, error))?,
      created_at: Timestamp::from_unix_millis(self.created_at),
      updated_at: Timestamp::from_unix_millis(self.updated_at),
    }))
  }
}

fn author(author: &str, agent: [Option<String>; 4]) -> Result<MemoryAuthor, StoreError> {
  match (author, agent) {
    ("person", [None, None, None, None]) => Ok(MemoryAuthor::Person),
    ("agent", [Some(profile), Some(run), Some(step), Some(execution)]) => Ok(MemoryAuthor::Agent(AgentProvenance {
      profile: parse("memory profile", &profile)?,
      run: parse("memory run", &run)?,
      step: StepKey::new(&step).map_err(|error| corrupt("memory step", error))?,
      execution: parse("memory execution", &execution)?,
    })),
    _ => Err(corrupt("memory record", "author does not match its provenance")),
  }
}

/// Reads the record `id`, if there is one.
pub(super) fn load(connection: &Connection, id: MemoryId) -> Result<Option<Memory>, StoreError> {
  let sql = format!("SELECT {COLUMNS} FROM memories m WHERE m.id = ?1");
  let mut statement = connection.prepare(&sql)?;
  let mut rows = statement.query_map([id.to_string()], Raw::from_row)?;
  rows.next().transpose()?.map(Raw::into_memory).transpose()
}

pub(super) fn insert(connection: &Connection, memory: &Memory) -> Result<(), StoreError> {
  let content = memory.content();
  let task = match memory.scope() {
    MemoryScope::Task(task) => Some(task.to_string()),
    MemoryScope::Project => None,
  };
  let (author, [profile, run, step, execution]) = match memory.author() {
    MemoryAuthor::Person => ("person", [None, None, None, None]),
    MemoryAuthor::Agent(agent) => (
      "agent",
      [
        Some(agent.profile.to_string()),
        Some(agent.run.to_string()),
        Some(agent.step.to_string()),
        Some(agent.execution.to_string()),
      ],
    ),
  };
  let sources = serde_json::to_string(memory.sources()).map_err(|_| StoreError::Unrepresentable("memory sources"))?;
  connection.execute(
    "INSERT INTO memories (id, task_id, kind, key, text, tags, importance, confidence, status, pinned, \
     supersedes_id, author, profile_id, run_id, step_key, step_execution_id, source_events, created_at, updated_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
    params![
      memory.id().to_string(),
      task,
      content.kind().name(),
      content.key().map(MemoryKey::as_str),
      content.text(),
      content.tags().joined(),
      content.importance().get(),
      content.confidence().percent(),
      memory.status().name(),
      memory.is_pinned(),
      memory.supersedes().map(|id| id.to_string()),
      author,
      profile,
      run,
      step,
      execution,
      sources,
      memory.created_at().unix_millis(),
      memory.updated_at().unix_millis(),
    ],
  )?;
  Ok(())
}

/// Stores what `memory` says, its status and pin, if the stored record's status is one of
/// `from`; the number of rows changed (0 or 1).
pub(super) fn update(connection: &Connection, memory: &Memory, from: &[MemoryStatus]) -> Result<usize, StoreError> {
  let content = memory.content();
  let statuses: Vec<&str> = from.iter().map(|status| status.name()).collect();
  let changed = connection.execute(
    "UPDATE memories SET kind = ?2, key = ?3, text = ?4, tags = ?5, importance = ?6, confidence = ?7, status = ?8, \
     pinned = ?9, updated_at = ?10 WHERE id = ?1 AND status IN (SELECT value FROM json_each(?11))",
    params![
      memory.id().to_string(),
      content.kind().name(),
      content.key().map(MemoryKey::as_str),
      content.text(),
      content.tags().joined(),
      content.importance().get(),
      content.confidence().percent(),
      memory.status().name(),
      memory.is_pinned(),
      memory.updated_at().unix_millis(),
      serde_json::to_string(&statuses).map_err(|_| StoreError::Unrepresentable("memory statuses"))?,
    ],
  )?;
  Ok(changed)
}
