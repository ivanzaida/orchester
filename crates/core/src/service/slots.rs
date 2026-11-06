//! A start that waits for *Services at once* slots (`docs/architecture/agent-testing.md`, decision 3; card `qsBGf` in
//! `oXVuM`): how many slots it needs under its project's share and the computer's limit, what it starts, how many of
//! the computer's slots are held and how many it could take now, and which runs hold them. The run's conversation
//! keeps it as a runner note, its names as plain text, so a note stored once always reads back; a note stored before
//! the holders and counts were kept has none of them.

use serde::{Deserialize, Serialize};

use crate::{id::ProjectId, time::Timestamp};

/// What a start waits for, said once when it begins to wait.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSlotWait {
  /// The slots it needs.
  pub slots: u32,
  /// Its project's share of the computer's slots.
  pub share: u32,
  /// The computer's *Services at once*.
  pub limit: u32,
  /// What it starts; `None` in notes stored before it was kept.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub starts: Option<SlotUse>,
  /// How many slots it could take now: free both among its project's share and among the computer's.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub free: Option<u32>,
  /// How many of the computer's slots are held now, by whichever runner.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub held: Option<u32>,
  /// Who holds them, in slot order, one entry per start; empty when no held slot names a holder that reads.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub holders: Vec<ServiceSlotHolder>,
}

/// What a start's slots are for: a stack's services, services with their needs, or a test suite's own process when
/// it names neither.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotUse {
  /// The stack it starts.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub stack: Option<String>,
  /// The services it starts, in the order they start.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub services: Vec<String>,
}

impl SlotUse {
  /// A test suite's own process.
  pub const fn suite() -> Self {
    Self {
      stack: None,
      services: Vec::new(),
    }
  }

  /// Whether it is a test suite's own process.
  pub const fn is_suite(&self) -> bool {
    self.stack.is_none() && self.services.is_empty()
  }
}

/// A start of another run that holds service slots: its project, task and run, the step it started in, what it
/// started, how many slots it holds and since when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSlotHolder {
  pub project: ProjectId,
  /// Its task's key, `PLT-38`.
  pub task: String,
  /// Its run's number in the project.
  pub run: u32,
  /// The step whose agent started it; `None` when the runner did not know it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub step: Option<SlotStep>,
  pub what: SlotUse,
  pub slots: u32,
  pub since: Timestamp,
}

/// A step of a run's pipeline: its position, from 1, and its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotStep {
  pub number: u32,
  pub name: String,
}
