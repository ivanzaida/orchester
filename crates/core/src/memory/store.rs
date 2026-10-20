//! Where memory records are kept. The project database implements it today (SQLite with full-text
//! indexes); a server will implement it with a store of its own.

use super::{Delivery, InjectionReason, Memory, MemoryChanges, MemoryQuery, MemorySettings, MemoryStatus};
use crate::{
  id::{InvocationId, MemoryId, RunId, StepExecutionId, TaskId},
  time::Timestamp,
};

/// Which scopes a listing covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeFilter {
  /// The records of one task.
  Task(TaskId),
  /// The project's records.
  Project,
  /// What an agent of the task may get: the task's records and the project's.
  TaskAndProject(TaskId),
  /// Every record: the project's and every task's.
  All,
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

/// A stored record that cannot be read back: its id as stored and what is wrong with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnreadableMemory {
  pub id: String,
  pub detail: String,
}

/// A record chosen for an invocation's prompt, why, and whether the prompt got it: in prompt order,
/// then the records the limits left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InjectedMemory {
  pub memory: MemoryId,
  pub reason: InjectionReason,
  /// Its estimated tokens ([`super::estimated_tokens`]) as given, or as it would have been.
  pub tokens: u32,
  pub delivery: Delivery,
}

/// An invocation whose prompt was given a record: which, in which run and step execution, when it
/// started, why it got the record and its estimated tokens there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GivenMemory {
  pub invocation: InvocationId,
  pub run: RunId,
  pub execution: StepExecutionId,
  pub started_at: Timestamp,
  pub reason: InjectionReason,
  pub tokens: u32,
  pub delivery: Delivery,
}

/// Memory records, the project's memory setting and what each invocation was given. Agents' writes
/// come from the runner; people's from the desktop.
pub trait MemoryStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Stores new records, atomically; a record whose id is stored already is refused. A key names at most one active
  /// record of its scope: a record whose key another active record of its scope has is refused, and nothing is
  /// stored.
  fn add_memories(&mut self, memories: &[Memory]) -> Result<(), Self::Error>;

  /// Stores `changes` atomically: the changed records first, then the added ones, under the same
  /// rules as [`MemoryStore::save_memory`] and [`MemoryStore::add_memories`].
  fn commit_memories(&mut self, changes: &mut MemoryChanges) -> Result<(), Self::Error>;

  /// Reads the records `filter` selects and stores the changes `change` makes of them, in one
  /// transaction that holds the write lock from the read on, so no other writer comes between.
  /// `change`'s own error comes back inside, with nothing stored.
  fn change_memories<E>(
    &mut self,
    filter: &MemoryFilter,
    change: impl FnOnce(Vec<Memory>) -> Result<MemoryChanges, E>,
  ) -> Result<Result<(), E>, Self::Error>;

  /// Stores what an existing record says, its status and its pin, under the same key rule. The
  /// stored record must still be at `memory`'s revision; a record changed since is refused as a
  /// conflict, with nothing stored. `memory` moves to the new revision.
  fn save_memory(&mut self, memory: &mut Memory) -> Result<(), Self::Error>;

  /// Stores `superseded` and its `replacement` (new, or a proposal just approved) after
  /// [`Memory::supersede`], atomically, under the revision rule of [`MemoryStore::save_memory`].
  /// Fails, changing nothing, when the stored record is no longer active.
  fn supersede_memory(&mut self, superseded: &mut Memory, replacement: &mut Memory) -> Result<(), Self::Error>;

  /// Removes a record for good, with its history: the earlier versions it superseded. Only a
  /// person deletes records; agents are never offered this. A record that superseded it no longer
  /// names it; invocations that were given it keep its id, which no longer resolves.
  fn delete_memory(&mut self, id: MemoryId) -> Result<(), Self::Error>;

  fn memory(&self, id: MemoryId) -> Result<Option<Memory>, Self::Error>;

  /// The records `filter` selects, oldest first. A record whose stored row cannot be read back is
  /// left out, here and in the search; [`MemoryStore::unreadable_memories`] lists them.
  fn memories(&self, filter: &MemoryFilter) -> Result<Vec<Memory>, Self::Error>;

  /// The records whose stored row cannot be read back, and why.
  fn unreadable_memories(&self) -> Result<Vec<UnreadableMemory>, Self::Error>;

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

  /// The invocations whose prompts were given record `memory` (whole or cut to fit), latest first.
  fn given_memory(&self, memory: MemoryId) -> Result<Vec<GivenMemory>, Self::Error>;
}
