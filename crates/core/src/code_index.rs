//! The code index of a project's repository as its agents get it (`docs/design/orchester-workspace.md`,
//! "Code index"): an index per git revision, built by the runner when a run starts from a revision
//! that has none, which a new or resumed agent is pointed at together with the internal skill that
//! explains how to search it with ripgrep.
//!
//! - Project settings › Code index › *Use code index* ([`CodeIndexSetting`]) is read when an agent starts: agents that
//!   start after a change follow it. It is on by default. Off, nothing is built and no agent gets an index or the
//!   skill.
//! - Each agent invocation records what it got ([`CodeIndexUse`]): the index of the run's base revision (with or
//!   without ripgrep, whose absence the skill's `git grep` fallback covers), nothing because the setting was off, or
//!   nothing because the index could not be built.

use serde::{Deserialize, Serialize};

use crate::revision::Revision;

/// Whether a project's agents get the code index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeIndexSetting {
  #[default]
  On,
  Off,
}

impl CodeIndexSetting {
  pub const fn is_on(self) -> bool {
    matches!(self, Self::On)
  }
}

/// What one agent invocation got of the code index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CodeIndexUse {
  /// The index of the run's base `revision` and the skill to search it; `ripgrep` says why the
  /// agent had no ripgrep to search with, if it had none.
  Given {
    revision: Revision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ripgrep: Option<RipgrepMissing>,
  },
  /// The project has the code index off.
  Off,
  /// The setting is on, but the agent got no index.
  Unavailable { reason: CodeIndexUnavailable },
}

/// Why an agent got no code index although the project has it on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum CodeIndexUnavailable {
  /// The index of the run's base revision could not be built; `detail` says why.
  BuildFailed { detail: String },
}

/// Why an agent has no ripgrep, which the index skill searches with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum RipgrepMissing {
  /// Found neither on the agent's `PATH` nor with an agent CLI.
  NotFound,
  /// Found but not given to the agent (copied into the project's tools folder); `detail` says why.
  Unavailable { detail: String },
}
