//! Full-text search of memory with the two FTS5 indexes of `memories`: word stems as prefixes in
//! `memories_words` (unicode61, BM25) and fragments as substrings in `memories_trigrams`. Only
//! active records of the task and of the project are searched.

use std::collections::BTreeMap;

use orchester_core::{
  id::{MemoryId, TaskId},
  memory::{MemoryMatch, MemoryQuery},
};
use rusqlite::{Connection, params};

use super::rows::{COLUMN_COUNT, COLUMNS, read};
use crate::StoreError;

/// Column weights of both indexes: a record's key and tags count twice its text.
const WEIGHTS: &str = "1.0, 2.0, 2.0";

pub(super) fn search(
  connection: &Connection,
  task: TaskId,
  query: &MemoryQuery,
  limit: usize,
) -> Result<Vec<MemoryMatch>, StoreError> {
  let mut found: BTreeMap<MemoryId, MemoryMatch> = BTreeMap::new();
  let words = query.words().iter().map(|word| format!("{}*", quoted(word)));
  for (memory, relevance) in matches(connection, "memories_words", &any(words), task, limit)? {
    found.entry(memory.id()).or_insert_with(|| empty(memory)).words = relevance;
  }
  let fragments = query.fragments().iter().map(|fragment| quoted(fragment));
  for (memory, relevance) in matches(connection, "memories_trigrams", &any(fragments), task, limit)? {
    found.entry(memory.id()).or_insert_with(|| empty(memory)).fragments = relevance;
  }
  let best_words = found.values().map(|found| found.words).fold(0.0, f64::max);
  let best_fragments = found.values().map(|found| found.fragments).fold(0.0, f64::max);
  let relevance = |found: &MemoryMatch| share(found.words, best_words) + share(found.fragments, best_fragments);
  let mut found: Vec<MemoryMatch> = found.into_values().collect();
  found.sort_by(|left, right| relevance(right).total_cmp(&relevance(left)));
  found.truncate(limit);
  Ok(found)
}

fn empty(memory: orchester_core::memory::Memory) -> MemoryMatch {
  MemoryMatch {
    memory,
    words: 0.0,
    fragments: 0.0,
  }
}

fn share(value: f64, best: f64) -> f64 {
  if best > 0.0 { value / best } else { 0.0 }
}

/// An FTS5 string: in double quotes, with double quotes doubled, so no term is read as syntax.
fn quoted(term: &str) -> String {
  format!("\"{}\"", term.replace('"', "\"\""))
}

fn any(terms: impl Iterator<Item = String>) -> String {
  terms.collect::<Vec<_>>().join(" OR ")
}

/// The active records of `task` and the project that `expression` matches in `index`, with their
/// BM25 relevance (positive, larger is better), best first.
fn matches(
  connection: &Connection,
  index: &str,
  expression: &str,
  task: TaskId,
  limit: usize,
) -> Result<Vec<(orchester_core::memory::Memory, f64)>, StoreError> {
  if expression.is_empty() {
    return Ok(Vec::new());
  }
  let sql = format!(
    "
    SELECT {COLUMNS}, -bm25({index}, {WEIGHTS}) AS relevance
    FROM {index} JOIN memories m ON m.seq = {index}.rowid
    WHERE {index} MATCH ?1
      AND m.status = 'active'
      AND (m.task_id = ?2 OR m.task_id IS NULL)
    ORDER BY relevance DESC
    LIMIT ?3"
  );
  let limit = i64::try_from(limit).unwrap_or(i64::MAX);
  let mut statement = connection.prepare(&sql)?;
  let rows = statement.query_map(params![expression, task.to_string(), limit], |row| {
    Ok((read(row)?, row.get::<_, f64>(COLUMN_COUNT)?))
  })?;
  let mut found = Vec::new();
  for row in rows {
    // A record that cannot be read back is left out; `unreadable_memories` lists it.
    if let ((_, Ok(memory)), relevance) = row? {
      found.push((memory, relevance));
    }
  }
  Ok(found)
}
