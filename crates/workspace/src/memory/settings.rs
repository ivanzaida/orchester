//! The Memory group of Project settings › Context: how agents' records are
//! saved in task and in project memory, and how much memory each agent invocation gets ([`MemorySettings`]). Reading
//! and saving block on SQLite; the UI calls them off the UI thread.

use std::path::Path;

use orchester_core::{
  memory::{MemoryLimits, MemorySettings, MemoryStore},
  tokens::digits_only,
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::Message;

pub fn load_in(repo_root: &Path) -> Result<MemorySettings, StoreError> {
  ProjectDb::open(repo_root)?.memory_settings()
}

/// Saves what `asked` changed from `base`, the settings the screen showed, onto the stored ones, so a field changed
/// elsewhere meanwhile keeps its change; the settings stored now.
pub fn save_in(repo_root: &Path, base: MemorySettings, asked: MemorySettings) -> Result<MemorySettings, StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  let stored = db.memory_settings()?;
  let records = pick(base.limits.records(), asked.limits.records(), stored.limits.records());
  let tokens = pick(base.limits.tokens(), asked.limits.tokens(), stored.limits.tokens());
  let saved = MemorySettings {
    task_writes: pick(base.task_writes, asked.task_writes, stored.task_writes),
    project_writes: pick(base.project_writes, asked.project_writes, stored.project_writes),
    limits: MemoryLimits::new(records, tokens).expect("each limit is one of two valid limits"),
  };
  db.save_memory_settings(&saved)?;
  Ok(saved)
}

/// The asked value where the screen changed it, else the stored one.
fn pick<T: PartialEq + Copy>(base: T, asked: T, stored: T) -> T {
  if asked == base { stored } else { asked }
}

/// *Records per invocation* as typed: a whole number in [`MemoryLimits::RECORDS`].
pub fn read_records(text: &str) -> Result<u16, Message> {
  text
    .trim()
    .parse()
    .ok()
    .filter(|records| MemoryLimits::RECORDS.contains(records))
    .ok_or_else(|| {
      Message::new("memory.settings.recordsInvalid")
        .with("min", MemoryLimits::RECORDS.start().to_string())
        .with("max", MemoryLimits::RECORDS.end().to_string())
    })
}

/// *Tokens per invocation* as typed, its thousands grouped with `separator` or not: a whole number in
/// [`MemoryLimits::TOKENS`].
pub fn read_tokens(text: &str, separator: &str) -> Result<u32, Message> {
  digits_only(text, separator)
    .and_then(|digits| digits.parse().ok())
    .filter(|tokens| MemoryLimits::TOKENS.contains(tokens))
    .ok_or_else(|| {
      Message::new("memory.settings.tokensInvalid")
        .with("min", grouped(*MemoryLimits::TOKENS.start(), separator))
        .with("max", grouped(*MemoryLimits::TOKENS.end(), separator))
    })
}

fn grouped(count: u32, separator: &str) -> String {
  orchester_core::tokens::grouped(u64::from(count), separator)
}
