//! Project settings › Context › Memory: how agents' writes are kept, for task memory and for project memory
//! separately, and how much memory an agent's prompt gets. Read at every invocation, so a change
//! applies to the next agent that starts; records already proposed stay proposed when a scope
//! switches to saving automatically.

use std::ops::RangeInclusive;

use thiserror::Error;

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

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MemoryLimitsError {
  #[error("records per invocation are from 1 to 50, not {0}")]
  Records(u16),
  #[error("tokens per invocation are from 500 to 20,000, not {0}")]
  Tokens(u32),
}

/// How much memory one agent invocation's prompt gets at most.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryLimits {
  records: u16,
  tokens: u32,
}

impl MemoryLimits {
  pub const RECORDS: RangeInclusive<u16> = 1..=50;
  pub const TOKENS: RangeInclusive<u32> = 500..=20_000;
  /// 8 records and 3,000 tokens.
  pub const DEFAULT: Self = Self {
    records: 8,
    tokens: 3_000,
  };

  pub fn new(records: u16, tokens: u32) -> Result<Self, MemoryLimitsError> {
    if !Self::RECORDS.contains(&records) {
      return Err(MemoryLimitsError::Records(records));
    }
    if !Self::TOKENS.contains(&tokens) {
      return Err(MemoryLimitsError::Tokens(tokens));
    }
    Ok(Self { records, tokens })
  }

  pub const fn records(self) -> u16 {
    self.records
  }

  /// Estimated tokens of the records' text ([`super::estimated_tokens`]).
  pub const fn tokens(self) -> u32 {
    self.tokens
  }
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
