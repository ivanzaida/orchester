//! Agent memory (`docs/design/orchester-workspace.md`, "Task and project memory"): versioned records
//! of what agents and people learned about a task or the project, kept apart from messages, run
//! events and artifacts. A record belongs to one task or to the project; an agent's prompt gets the
//! active records that matter for its step ([`recall`]), within the project's limits.
//!
//! A record changes only through the transitions here: a proposed record is approved or rejected;
//! an active one is edited, pinned, deprecated or superseded by a newer record that names it. Only
//! active records reach prompts. Deleting a record is a person's decision
//! ([`MemoryStore::delete_memory`]).

mod content;
mod handoff;
mod query;
mod recall;
mod screen;
mod settings;
mod store;

use std::fmt;

use thiserror::Error;

pub use self::{
  content::{Confidence, Importance, MemoryContent, MemoryContentError, MemoryKey, MemoryKind, Tags},
  handoff::Handoff,
  query::MemoryQuery,
  recall::{InjectionReason, RecallRequest, Recalled, RecalledMemory, estimated_tokens, recall},
  screen::{ContainsSecret, ScreenUnavailable, ScreenedContent, SecretScreen},
  settings::{MemoryLimits, MemorySettings, WritePolicy},
  store::{InjectedMemory, MemoryFilter, MemoryMatch, MemoryStore, ScopeFilter},
};
use crate::{
  id::{MemoryId, ResourceId, RunId, StepExecutionId, TaskId},
  resource::StepKey,
  time::Timestamp,
};

/// Whom a record is for: the agents of one task, or of every task of the project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryScope {
  Task(TaskId),
  Project,
}

/// Where a record stands. Only [`MemoryStatus::Active`] records reach agents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryStatus {
  /// Written by an agent while its scope's writes are saved after approval; waits for a person.
  Proposed,
  Active,
  /// A newer record replaced it.
  Superseded,
  /// No longer true or wanted, or a proposal a person rejected.
  Deprecated,
}

impl MemoryStatus {
  pub const ALL: [Self; 4] = [Self::Proposed, Self::Active, Self::Superseded, Self::Deprecated];

  /// Stable name, as stored.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Proposed => "proposed",
      Self::Active => "active",
      Self::Superseded => "superseded",
      Self::Deprecated => "deprecated",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|status| status.name() == name)
  }
}

impl fmt::Display for MemoryStatus {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.name())
  }
}

/// The agent that wrote a record: its profile, and the run, step and execution it worked in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentProvenance {
  pub profile: ResourceId,
  pub run: RunId,
  pub step: StepKey,
  pub execution: StepExecutionId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemoryAuthor {
  Agent(AgentProvenance),
  Person,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MemoryError {
  #[error("memory {id} is {status}; only a proposed record is approved or rejected")]
  NotProposed { id: MemoryId, status: MemoryStatus },
  #[error("memory {id} is {status}; only an active record is deprecated or superseded")]
  NotActive { id: MemoryId, status: MemoryStatus },
  #[error("memory {id} is {status} and no longer changes")]
  Closed { id: MemoryId, status: MemoryStatus },
}

/// A record as a store keeps it; [`Memory::from_stored`] reads it back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredMemory {
  pub id: MemoryId,
  pub scope: MemoryScope,
  pub author: MemoryAuthor,
  pub content: ScreenedContent,
  pub status: MemoryStatus,
  pub pinned: bool,
  pub supersedes: Option<MemoryId>,
  pub sources: Vec<u64>,
  pub created_at: Timestamp,
  pub updated_at: Timestamp,
}

/// One memory record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
  id: MemoryId,
  scope: MemoryScope,
  author: MemoryAuthor,
  content: MemoryContent,
  status: MemoryStatus,
  pinned: bool,
  supersedes: Option<MemoryId>,
  sources: Vec<u64>,
  created_at: Timestamp,
  updated_at: Timestamp,
}

impl Memory {
  /// A record an agent writes to `scope`, kept as the project's `settings` say for that scope:
  /// active at once, or proposed until a person approves it. `sources` are the sequences of the
  /// run's transcript entries it came from.
  pub fn by_agent(
    scope: MemoryScope,
    provenance: AgentProvenance,
    content: ScreenedContent,
    sources: Vec<u64>,
    settings: &MemorySettings,
    now: Timestamp,
  ) -> Self {
    let status = match settings.policy(scope) {
      WritePolicy::Automatic => MemoryStatus::Active,
      WritePolicy::AfterApproval => MemoryStatus::Proposed,
    };
    Self::created(scope, MemoryAuthor::Agent(provenance), content, status, sources, now)
  }

  /// A record a person writes: active at once.
  pub fn by_person(scope: MemoryScope, content: ScreenedContent, now: Timestamp) -> Self {
    Self::created(
      scope,
      MemoryAuthor::Person,
      content,
      MemoryStatus::Active,
      Vec::new(),
      now,
    )
  }

  fn created(
    scope: MemoryScope,
    author: MemoryAuthor,
    content: ScreenedContent,
    status: MemoryStatus,
    sources: Vec<u64>,
    now: Timestamp,
  ) -> Self {
    Self {
      id: MemoryId::generate(),
      scope,
      author,
      content: content.into_content(),
      status,
      pinned: false,
      supersedes: None,
      sources,
      created_at: now,
      updated_at: now,
    }
  }

  pub fn from_stored(stored: StoredMemory) -> Self {
    Self {
      id: stored.id,
      scope: stored.scope,
      author: stored.author,
      content: stored.content.into_content(),
      status: stored.status,
      pinned: stored.pinned,
      supersedes: stored.supersedes,
      sources: stored.sources,
      created_at: stored.created_at,
      updated_at: stored.updated_at,
    }
  }

  /// A person approves a proposed record: it becomes active and reaches the next agents.
  pub fn approve(&mut self, now: Timestamp) -> Result<(), MemoryError> {
    self.decide(MemoryStatus::Active, now)
  }

  /// A person rejects a proposed record: it is deprecated and never reaches an agent.
  pub fn reject(&mut self, now: Timestamp) -> Result<(), MemoryError> {
    self.decide(MemoryStatus::Deprecated, now)
  }

  fn decide(&mut self, status: MemoryStatus, now: Timestamp) -> Result<(), MemoryError> {
    if self.status != MemoryStatus::Proposed {
      return Err(MemoryError::NotProposed {
        id: self.id,
        status: self.status,
      });
    }
    self.status = status;
    self.updated_at = now;
    Ok(())
  }

  /// An active record is no longer true or wanted; it stops reaching agents.
  pub fn deprecate(&mut self, now: Timestamp) -> Result<(), MemoryError> {
    self.require_active()?;
    self.status = MemoryStatus::Deprecated;
    self.pinned = false;
    self.updated_at = now;
    Ok(())
  }

  /// Replaces what an open record says in place, e.g. to fix a typo; a record whose meaning
  /// changes is superseded instead, so the earlier one stays readable.
  pub fn edit(&mut self, content: ScreenedContent, now: Timestamp) -> Result<(), MemoryError> {
    self.require_open()?;
    self.content = content.into_content();
    self.updated_at = now;
    Ok(())
  }

  /// Pins an open record, so every agent of its scope gets it first, or unpins it.
  pub fn set_pinned(&mut self, pinned: bool, now: Timestamp) -> Result<(), MemoryError> {
    self.require_open()?;
    self.pinned = pinned;
    self.updated_at = now;
    Ok(())
  }

  /// A person replaces this active record with `content`: the new record of the same scope names
  /// it and takes over its pin, and this one is superseded. Store both with
  /// [`MemoryStore::supersede_memory`].
  pub fn supersede(&mut self, content: ScreenedContent, now: Timestamp) -> Result<Self, MemoryError> {
    self.require_active()?;
    let mut replacement = Self::by_person(self.scope, content, now);
    replacement.supersedes = Some(self.id);
    replacement.pinned = self.pinned;
    self.status = MemoryStatus::Superseded;
    self.pinned = false;
    self.updated_at = now;
    Ok(replacement)
  }

  fn require_active(&self) -> Result<(), MemoryError> {
    if self.status == MemoryStatus::Active {
      return Ok(());
    }
    Err(MemoryError::NotActive {
      id: self.id,
      status: self.status,
    })
  }

  fn require_open(&self) -> Result<(), MemoryError> {
    if matches!(self.status, MemoryStatus::Proposed | MemoryStatus::Active) {
      return Ok(());
    }
    Err(MemoryError::Closed {
      id: self.id,
      status: self.status,
    })
  }

  pub const fn id(&self) -> MemoryId {
    self.id
  }

  pub const fn scope(&self) -> MemoryScope {
    self.scope
  }

  pub const fn author(&self) -> &MemoryAuthor {
    &self.author
  }

  pub const fn content(&self) -> &MemoryContent {
    &self.content
  }

  pub const fn status(&self) -> MemoryStatus {
    self.status
  }

  pub const fn is_pinned(&self) -> bool {
    self.pinned
  }

  /// The record this one replaced.
  pub const fn supersedes(&self) -> Option<MemoryId> {
    self.supersedes
  }

  /// Sequences of the run transcript entries the record came from.
  pub fn sources(&self) -> &[u64] {
    &self.sources
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  pub const fn updated_at(&self) -> Timestamp {
    self.updated_at
  }
}
