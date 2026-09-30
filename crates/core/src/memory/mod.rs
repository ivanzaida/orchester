//! Agent memory (`docs/design/orchester-workspace.md`, "Task and project memory"): versioned records
//! of what agents and people learned about a task or the project, kept apart from messages, run
//! events and artifacts. A record belongs to one task or to the project; an agent's prompt gets the
//! active records that matter for its step ([`recall`]), within the project's limits.
//!
//! A record changes only through the transitions of [`Memory`]: a proposed record is approved (and
//! then supersedes the record it replaces) or rejected; a person edits an open record in place, pins, deprecates and
//! restores it; an agent's newer record supersedes the one it names. Only active records reach prompts. Deleting a
//! record, with its history, is a person's decision ([`MemoryStore::delete_memory`]).

mod approval;
mod content;
mod handoff;
mod next;
mod query;
mod recall;
mod record;
mod render;
mod screen;
mod selection;
mod settings;
mod store;

use std::fmt;

pub use self::{
  approval::approval,
  content::{Confidence, Importance, MemoryContent, MemoryContentError, MemoryKey, MemoryKind, Tags},
  handoff::{FAILED_ATTEMPT_TAG, Handoff, HandoffWrite, MemoryChanges, StepFate},
  next::{StepContext, next_invocation_memory},
  query::MemoryQuery,
  recall::{InjectionReason, RecallRequest, estimated_tokens, recall},
  record::{AgentWrite, Memory, MemoryError, StoredMemory, StoredMemoryError},
  render::quoted,
  screen::{ContainsSecret, ScreenUnavailable, ScreenedContent, SecretScreen},
  selection::{Delivery, LeftOut, Recalled, RecalledMemory},
  settings::{MemoryLimits, MemoryLimitsError, MemorySettings, WritePolicy},
  store::{InjectedMemory, MemoryFilter, MemoryMatch, MemoryStore, ScopeFilter, UnreadableMemory},
};
use crate::{
  id::{ResourceId, RunId, StepExecutionId, TaskId},
  resource::StepKey,
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
  /// No longer true or wanted; a person may restore it.
  Deprecated,
  /// A proposal a person rejected; it never reaches an agent and is not restored.
  Rejected,
  /// A proposal its author replaced with a newer proposal before any person decided on it. It was
  /// never a version of anything: it is no one's history.
  Withdrawn,
}

impl MemoryStatus {
  pub const ALL: [Self; 6] = [
    Self::Proposed,
    Self::Active,
    Self::Superseded,
    Self::Deprecated,
    Self::Rejected,
    Self::Withdrawn,
  ];

  /// Stable name, as stored.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Proposed => "proposed",
      Self::Active => "active",
      Self::Superseded => "superseded",
      Self::Deprecated => "deprecated",
      Self::Rejected => "rejected",
      Self::Withdrawn => "withdrawn",
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
