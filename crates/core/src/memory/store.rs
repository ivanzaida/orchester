//! Where memory records are kept. The project database implements it today (SQLite with full-text
//! indexes); a server will implement it with a store of its own.

use super::{InjectionReason, Memory, MemoryQuery, MemorySettings, MemoryStatus};
use crate::id::{InvocationId, MemoryId, TaskId};

/// Which scopes a listing covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeFilter {
  /// The records of one task.
  Task(TaskId),
  /// The project's records.
  Project,
  /// What an agent of the task may get: the task's records and the project's.
  TaskAndProject(TaskId),
}

/// A listing's scopes, and the status and pin its records have, if it asks for one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryFilter {
  pub scope: ScopeFilter,
  pub status: Option<MemoryStatus>,
  pub pinned_only: bool,
}

/// A record found by a search and how well it matched: the BM25 relevance of its words and of its
/// trigram fragments, each positive and larger for a better match; zero where it did not match.
#[derive(Clone, Debug, PartialEq)]
pub struct MemoryMatch {
  pub memory: Memory,
  pub words: f64,
  pub fragments: f64,
}

/// A record an invocation's prompt held, and why it was chosen, in prompt order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InjectedMemory {
  pub memory: MemoryId,
  pub reason: InjectionReason,
  /// Its estimated tokens ([`super::estimated_tokens`]) as injected.
  pub tokens: u32,
}

/// Memory records, the project's memory setting and what each invocation was given. Agents' writes
/// come from the runner; people's from the desktop.
pub trait MemoryStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores new records, atomically. A key names at most one active record of its scope: a record
  /// whose key another active record of its scope has is refused, and nothing is stored.
  fn add_memories(&mut self, memories: &[Memory]) -> Result<(), Self::Error>;

  /// Stores what an existing record says, its status and its pin, under the same key rule.
  fn save_memory(&mut self, memory: &Memory) -> Result<(), Self::Error>;

  /// Stores `superseded` and its `replacement` (new, or a proposal just approved) after
  /// [`Memory::supersede`], atomically. Fails, changing nothing, when the stored record is no
  /// longer active.
  fn supersede_memory(&mut self, superseded: &Memory, replacement: &Memory) -> Result<(), Self::Error>;

  /// Removes a record for good, with its history: the earlier versions it superseded. Only a
  /// person deletes records; agents are never offered this. A record that superseded it no longer
  /// names it; invocations that were given it keep its id, which no longer resolves.
  fn delete_memory(&mut self, id: MemoryId) -> Result<(), Self::Error>;

  fn memory(&self, id: MemoryId) -> Result<Option<Memory>, Self::Error>;

  /// The records `filter` selects, oldest first.
  fn memories(&self, filter: &MemoryFilter) -> Result<Vec<Memory>, Self::Error>;

  /// The active records of `task` and of the project that match `query`, best first, at most
  /// `limit` of them.
  fn search_memories(&self, task: TaskId, query: &MemoryQuery, limit: usize) -> Result<Vec<MemoryMatch>, Self::Error>;

  /// The project's memory setting: the defaults until it was changed.
  fn memory_settings(&self) -> Result<MemorySettings, Self::Error>;

  fn save_memory_settings(&mut self, settings: &MemorySettings) -> Result<(), Self::Error>;

  /// Records what `invocation`'s prompt was given, once, before its agent starts.
  fn save_injected_memory(&mut self, invocation: InvocationId, injected: &[InjectedMemory]) -> Result<(), Self::Error>;

  /// What `invocation`'s prompt was given, in prompt order; empty when it got nothing.
  fn injected_memory(&self, invocation: InvocationId) -> Result<Vec<InjectedMemory>, Self::Error>;
}
