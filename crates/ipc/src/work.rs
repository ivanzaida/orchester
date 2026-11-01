//! What a runner is working on, sent as [`crate::Event::WorkSummary`]: the status bar's Agents
//! count and the rows of the close confirmation (02.C `Xz30C`, 02.D `a0tTV`), and the ready steps
//! that wait for one of its agent slots.

use std::num::NonZeroU32;

use orchester_core::{
  connection::AgentProvider,
  id::{RunId, StepExecutionId, TaskId},
};
use serde::{Deserialize, Serialize};

/// The runs a runner executes, its agent slots and the steps waiting for one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkSummary {
  pub agent_slots: NonZeroU32,
  pub runs: Vec<RunWork>,
  pub slot_waits: Vec<SlotWait>,
}

impl WorkSummary {
  /// Agents at work: runs that have not settled.
  pub fn agents_working(&self) -> usize {
    self
      .runs
      .iter()
      .filter(|run| !matches!(run.state, WorkState::Saved { .. }))
      .count()
  }

  /// Agents at work, and the slots they share.
  pub fn agents(&self) -> (usize, NonZeroU32) {
    (self.agents_working(), self.agent_slots)
  }

  /// Whether `execution` is ready and held back only because every agent slot is in use.
  pub fn waits_for_slot(&self, execution: StepExecutionId) -> bool {
    self.slot_waits.iter().any(|wait| wait.execution == execution)
  }

  /// How many ready steps wait for an agent slot.
  pub fn steps_waiting_for_slot(&self) -> usize {
    self.slot_waits.len()
  }
}

/// A ready agent step that only waits for a free agent slot. The runner knows this and nothing stores
/// it: the step stays pending in the run's log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotWait {
  pub run: RunId,
  pub execution: StepExecutionId,
}

/// A run with an agent at work, with what a person needs to recognize it. The display values
/// are runtime data of the project: task, step, agent role and model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunWork {
  pub run: RunId,
  pub task: TaskId,
  pub task_key: String,
  pub task_title: String,
  pub step: String,
  pub agent: String,
  pub provider: AgentProvider,
  pub model: String,
  pub state: WorkState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkState {
  /// Its agent works.
  Working,
  /// The runner is stopping at checkpoints: the agent finishes its current work first.
  Saving,
  /// The run settled while the runner stopped at checkpoints: paused at `checkpoint` (a
  /// 40-character revision), paused without a new one, or ended on its own.
  Saved { checkpoint: Option<String> },
}
