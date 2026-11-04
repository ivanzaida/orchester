//! The memory area of the coordinator link: the task and project memory an agent's prompt gets, chosen where the
//! records are (the coordinator searches the project database's indexes within the project's Memory setting), the
//! records that cannot be read back, and the Memory setting itself. Records are only read here: new records and
//! their changes go through the coordinator otherwise.

use orchester_core::{
  memory::{MemorySettings, Recalled, StepContext, StepRecall, UnreadableMemory},
  time::Timestamp,
};
use serde::{Deserialize, Serialize};

use super::LinkError;

/// The coordinator's answers about the project's memory.
pub trait MemoryLink {
  /// The records the next invocation of the step `request` names gets, in prompt order, and those the limits left
  /// out, each with why it was chosen ([`orchester_core::memory::step_memory`]).
  fn recall(&self, request: &RecallRequest) -> Result<Recalled, LinkError>;

  /// The memory records whose stored row cannot be read back, and why.
  fn unreadable(&self, request: &UnreadableRequest) -> Result<UnreadableMemories, LinkError>;

  /// The project's Memory setting as it is now.
  fn settings(&self, request: &SettingsRequest) -> Result<MemorySettings, LinkError>;
}

/// The records of a step's next invocation, as they stand at `at`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallRequest {
  pub step: StepRecall,
  pub at: Timestamp,
}

impl RecallRequest {
  /// The request for the step of `context`, at `at`.
  pub fn for_step(context: StepContext<'_>, at: Timestamp) -> Self {
    Self {
      step: context.recall(),
      at,
    }
  }
}

/// The memory records that cannot be read back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnreadableRequest;

/// The records that cannot be read back, in the order they were stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnreadableMemories {
  pub records: Vec<UnreadableMemory>,
}

/// The project's Memory setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsRequest;
