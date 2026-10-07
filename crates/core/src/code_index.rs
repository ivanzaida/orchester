//! The code index of a project's repository as its agents get it (`docs/design/orchester-workspace.md`,
//! "Code index"): an index per git revision, built by the runner when a run starts from a revision
//! that has none, which a new or resumed agent is pointed at together with the internal skill that
//! explains how to search it with ripgrep.
//!
//! - Project settings › Code index › *Use code index* ([`CodeIndexSetting`]) is read when an agent starts: agents that
//!   start after a change follow it. It is off until a person turns it on: measured with Claude Code and Orchester's
//!   file tools, agents hardly used the index and runs with it cost more
//!   (`docs/evidence/code-index-measurement-2026-10-07.md`). Off, nothing is built and no agent gets an index or the
//!   skill.
//! - Each agent invocation records what it got ([`CodeIndexUse`]): the index of the run's base revision (with or
//!   without ripgrep, whose absence the skill's `git grep` fallback covers), nothing because the setting was off, or
//!   nothing because the index could not be built.

use serde::{Deserialize, Serialize};

use crate::{connection::AgentProvider, revision::Revision};

/// Whether a project's agents get the code index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeIndexSetting {
  On,
  #[default]
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
  /// The setting was turned on while the run executed, and the index was still being built when
  /// the agent started; agents that start after the build get it.
  Building,
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

/// How an agent without ripgrep searches instead, by its CLI and whether it got the code index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RipgrepFallback {
  /// Recorded before this was kept: not known.
  #[default]
  Unknown,
  /// Codex with the code index: it searches the index and the files with `git grep`.
  IndexWithGitGrep,
  /// Claude Code or a native agent with the code index: it searches with its own tools.
  IndexWithOwnTools,
  /// Codex without the code index: it searches the files with `git grep`.
  GitGrep,
  /// Claude Code or a native agent without the code index: it searches with its own tools.
  OwnTools,
}

impl RipgrepFallback {
  /// The fallback of an agent on `provider` that got the code index when `index_given`. A native
  /// agent's search tool falls back to a built-in search (`docs/architecture/native-agent.md` §4.3).
  pub const fn of(provider: AgentProvider, index_given: bool) -> Self {
    match (provider, index_given) {
      (AgentProvider::CodexCli, true) => Self::IndexWithGitGrep,
      (AgentProvider::CodexCli, false) => Self::GitGrep,
      (AgentProvider::ClaudeCodeCli | AgentProvider::AnthropicApi | AgentProvider::OpenAiApi, true) => {
        Self::IndexWithOwnTools
      }
      (AgentProvider::ClaudeCodeCli | AgentProvider::AnthropicApi | AgentProvider::OpenAiApi, false) => Self::OwnTools,
    }
  }
}
