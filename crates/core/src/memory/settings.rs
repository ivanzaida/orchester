//! Project settings › Memory: how agents' writes are kept, for task memory and for project memory
//! separately, and how much memory an agent's prompt gets. Read at every invocation, so a change
//! applies to the next agent that starts.

use std::num::{NonZeroU16, NonZeroU32};

use super::MemoryScope;

/// How a record an agent writes is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritePolicy {
  /// Saved automatically: active at once, and injected from the next invocation on.
  Automatic,
  /// Saved after approval: stored as proposed and never injected until a person approves it.
  AfterApproval,
}

impl WritePolicy {
  /// Stable name, as stored.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Automatic => "automatic",
      Self::AfterApproval => "after_approval",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    [Self::Automatic, Self::AfterApproval]
      .into_iter()
      .find(|policy| policy.name() == name)
  }
}

/// How much memory one agent invocation's prompt gets at most.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryLimits {
  pub records: NonZeroU16,
  /// Estimated tokens of the records' text ([`super::estimated_tokens`]).
  pub tokens: NonZeroU32,
}

impl MemoryLimits {
  pub const DEFAULT: Self = Self {
    records: NonZeroU16::new(8).expect("8 is above zero"),
    tokens: NonZeroU32::new(3_000).expect("3,000 is above zero"),
  };
}

/// A project's memory setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemorySettings {
  pub task_writes: WritePolicy,
  pub project_writes: WritePolicy,
  pub limits: MemoryLimits,
}

impl MemorySettings {
  /// Task memory is saved automatically, project memory after approval; 8 records and 3,000
  /// tokens per invocation.
  pub const DEFAULT: Self = Self {
    task_writes: WritePolicy::Automatic,
    project_writes: WritePolicy::AfterApproval,
    limits: MemoryLimits::DEFAULT,
  };

  /// How agents' writes to `scope` are kept.
  pub const fn policy(&self, scope: MemoryScope) -> WritePolicy {
    match scope {
      MemoryScope::Task(_) => self.task_writes,
      MemoryScope::Project => self.project_writes,
    }
  }
}

impl Default for MemorySettings {
  fn default() -> Self {
    Self::DEFAULT
  }
}
