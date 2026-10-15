//! A change the Orchestrator's agent proposed that only a person carries out (`docs/design/orchestrator-toolset.md`,
//! "Decisions" 3 and 4): publishing a draft, canceling a run or merging one. It holds what the person reads before
//! *Apply* and what *Apply* would change; a draft saved again, a version published meanwhile or a run with newer
//! work needs a fresh proposal.

use serde::{Deserialize, Serialize};

use crate::{
  id::{ResourceId, RunId},
  time::Timestamp,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedChange {
  /// What it changes as people read it: the draft's name, or the title of the run's task.
  pub subject: String,
  pub change: Change,
}

/// What a proposed change would do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
  /// Publish the draft of `resource`, of kind `of`, as version `version`: the draft as it was saved at `saved`.
  Publish {
    of: Publishing,
    resource: ResourceId,
    version: u32,
    saved: Timestamp,
  },
  /// Cancel run `run`, `#number`.
  Cancel { run: RunId, number: u32 },
  /// Merge run `run`, `#number`, holding what its last checkpoint `checkpoint` (a commit) holds.
  Merge {
    run: RunId,
    number: u32,
    checkpoint: Option<String>,
  },
}

/// What kind of draft a publication publishes, and so which of Orchester's tools publishes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Publishing {
  Pipeline,
  Profile,
  Instruction,
  Skill,
  Tool,
}

impl Publishing {
  pub const ALL: [Self; 5] = [
    Self::Pipeline,
    Self::Profile,
    Self::Instruction,
    Self::Skill,
    Self::Tool,
  ];

  /// The tool that publishes it (`publish_pipeline`, …).
  pub const fn tool(self) -> &'static str {
    match self {
      Self::Pipeline => "publish_pipeline",
      Self::Profile => "publish_profile",
      Self::Instruction => "publish_instruction",
      Self::Skill => "publish_skill",
      Self::Tool => "publish_tool",
    }
  }

  /// The argument that tool names the draft's resource by (`pipeline`, …).
  pub const fn argument(self) -> &'static str {
    match self {
      Self::Pipeline => "pipeline",
      Self::Profile => "profile",
      Self::Instruction => "instruction",
      Self::Skill => "skill",
      Self::Tool => "tool",
    }
  }

  /// The kind published by tool `name`; `None` for a tool that publishes nothing.
  pub fn of_tool(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|kind| kind.tool() == name)
  }
}
