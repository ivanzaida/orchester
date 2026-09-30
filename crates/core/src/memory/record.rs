//! One memory record and its transitions.

use thiserror::Error;

use super::{
  AgentProvenance, Confidence, MemoryAuthor, MemoryContent, MemoryScope, MemorySettings, MemoryStatus, ScreenedContent,
  WritePolicy,
};
use crate::{id::MemoryId, time::Timestamp};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MemoryError {
  #[error("memory {id} is {status}; only a proposed record is approved or rejected")]
  NotProposed { id: MemoryId, status: MemoryStatus },
  #[error("memory {id} is {status}; only an active record is deprecated or superseded")]
  NotActive { id: MemoryId, status: MemoryStatus },
  #[error("memory {id} is {status}; only a deprecated record is restored")]
  NotDeprecated { id: MemoryId, status: MemoryStatus },
  #[error("memory {id} is {status} and no longer changes")]
  Closed { id: MemoryId, status: MemoryStatus },
  /// A record supersedes only the active record of its own scope that it names, once it is active
  /// itself.
  #[error("memory {replacement} does not replace memory {id}")]
  NotItsReplacement { id: MemoryId, replacement: MemoryId },
}

/// What an agent's write carries besides its content: who wrote it, how sure it is, and the
/// sequences of the run transcript entries it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentWrite {
  pub provenance: AgentProvenance,
  pub confidence: Confidence,
  pub sources: Vec<u64>,
}

/// A record as a store keeps it; [`Memory::from_stored`] reads it back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredMemory {
  pub id: MemoryId,
  pub scope: MemoryScope,
  pub author: MemoryAuthor,
  pub content: ScreenedContent,
  pub confidence: Confidence,
  pub status: MemoryStatus,
  pub pinned: bool,
  pub supersedes: Option<MemoryId>,
  pub sources: Vec<u64>,
  pub created_at: Timestamp,
  pub updated_at: Timestamp,
  pub edited_at: Option<Timestamp>,
}

/// One memory record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
  id: MemoryId,
  scope: MemoryScope,
  author: MemoryAuthor,
  content: MemoryContent,
  confidence: Confidence,
  status: MemoryStatus,
  pinned: bool,
  supersedes: Option<MemoryId>,
  sources: Vec<u64>,
  created_at: Timestamp,
  updated_at: Timestamp,
  edited_at: Option<Timestamp>,
}

impl Memory {
  /// A record an agent writes to `scope`, kept as the project's `settings` say for that scope:
  /// active at once, or proposed until a person approves it.
  pub fn by_agent(
    scope: MemoryScope,
    write: AgentWrite,
    content: ScreenedContent,
    settings: &MemorySettings,
    now: Timestamp,
  ) -> Self {
    let status = match settings.policy(scope) {
      WritePolicy::Automatic => MemoryStatus::Active,
      WritePolicy::AfterApproval => MemoryStatus::Proposed,
    };
    Self {
      author: MemoryAuthor::Agent(write.provenance),
      confidence: write.confidence,
      sources: write.sources,
      ..Self::created(scope, content, status, now)
    }
  }

  /// A record an agent writes to replace `old`, an active record: of its scope, naming it, and
  /// kept as [`Memory::by_agent`] keeps records. It supersedes `old` once it is active
  /// ([`Memory::supersede`]).
  pub fn replacing(
    old: &Self,
    write: AgentWrite,
    content: ScreenedContent,
    settings: &MemorySettings,
    now: Timestamp,
  ) -> Result<Self, MemoryError> {
    old.require_active()?;
    let mut replacement = Self::by_agent(old.scope, write, content, settings, now);
    replacement.supersedes = Some(old.id);
    Ok(replacement)
  }

  /// A record a person writes: active at once and certain.
  pub fn by_person(scope: MemoryScope, content: ScreenedContent, now: Timestamp) -> Self {
    Self::created(scope, content, MemoryStatus::Active, now)
  }

  fn created(scope: MemoryScope, content: ScreenedContent, status: MemoryStatus, now: Timestamp) -> Self {
    Self {
      id: MemoryId::generate(),
      scope,
      author: MemoryAuthor::Person,
      content: content.into_content(),
      confidence: Confidence::High,
      status,
      pinned: false,
      supersedes: None,
      sources: Vec::new(),
      created_at: now,
      updated_at: now,
      edited_at: None,
    }
  }

  pub fn from_stored(stored: StoredMemory) -> Self {
    Self {
      id: stored.id,
      scope: stored.scope,
      author: stored.author,
      content: stored.content.into_content(),
      confidence: stored.confidence,
      status: stored.status,
      pinned: stored.pinned,
      supersedes: stored.supersedes,
      sources: stored.sources,
      created_at: stored.created_at,
      updated_at: stored.updated_at,
      edited_at: stored.edited_at,
    }
  }

  /// A person approves a proposed record: it becomes active and reaches the next agents. One that
  /// names a record it replaces then supersedes it ([`Memory::supersede`]).
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

  /// An active record is no longer true or wanted; it stops reaching agents and loses its pin.
  pub fn deprecate(&mut self, now: Timestamp) -> Result<(), MemoryError> {
    self.require_active()?;
    self.status = MemoryStatus::Deprecated;
    self.pinned = false;
    self.updated_at = now;
    Ok(())
  }

  /// A person takes a deprecated record back: it is active again.
  pub fn restore(&mut self, now: Timestamp) -> Result<(), MemoryError> {
    if self.status != MemoryStatus::Deprecated {
      return Err(MemoryError::NotDeprecated {
        id: self.id,
        status: self.status,
      });
    }
    self.status = MemoryStatus::Active;
    self.updated_at = now;
    Ok(())
  }

  /// A person changes what an open record says, in place: its author stays, and it is marked as
  /// edited by a person.
  pub fn edit(&mut self, content: ScreenedContent, now: Timestamp) -> Result<(), MemoryError> {
    self.require_open()?;
    self.content = content.into_content();
    self.updated_at = now;
    self.edited_at = Some(now);
    Ok(())
  }

  /// Pins an open record, so every agent of its scope gets it, or unpins it.
  pub fn set_pinned(&mut self, pinned: bool, now: Timestamp) -> Result<(), MemoryError> {
    self.require_open()?;
    self.pinned = pinned;
    self.updated_at = now;
    Ok(())
  }

  /// This active record is superseded by `replacement`, an active record that names it; the
  /// replacement takes over its pin. Store both with [`super::MemoryStore::supersede_memory`].
  pub fn supersede(&mut self, replacement: &mut Self, now: Timestamp) -> Result<(), MemoryError> {
    self.require_active()?;
    if replacement.supersedes != Some(self.id)
      || replacement.status != MemoryStatus::Active
      || replacement.scope != self.scope
    {
      return Err(MemoryError::NotItsReplacement {
        id: self.id,
        replacement: replacement.id,
      });
    }
    replacement.pinned |= self.pinned;
    self.status = MemoryStatus::Superseded;
    self.pinned = false;
    self.updated_at = now;
    Ok(())
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

  /// How sure its author is: an agent's own say, which people do not change.
  pub const fn confidence(&self) -> Confidence {
    self.confidence
  }

  pub const fn status(&self) -> MemoryStatus {
    self.status
  }

  pub const fn is_pinned(&self) -> bool {
    self.pinned
  }

  /// The record this one replaces.
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

  /// When a person last edited what it says; `None` while it says what its author wrote.
  pub const fn edited_at(&self) -> Option<Timestamp> {
    self.edited_at
  }
}
