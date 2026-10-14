//! A start of a task the Orchestrator's agent proposed: what the person reads before *Start* (the task's title, the
//! opening of its brief, the pipeline), and what it would run then ([`StartBasis`]). *Start* runs only that: a task
//! changed since, or a pipeline published anew, needs a fresh proposal.

use serde::{Deserialize, Serialize};

use crate::id::ResourceId;

/// Characters of the brief a proposal shows.
pub const BRIEF_CHARS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedStart {
  /// The pipeline's name, as the tool gave it.
  pub pipeline: String,
  pub title: String,
  /// The brief's opening, at most [`BRIEF_CHARS`] characters.
  pub brief: String,
  pub basis: StartBasis,
}

/// What a proposed start would run: the task's revision, its pipeline and the pipeline version it would start on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartBasis {
  pub revision: u64,
  pub pipeline: Option<ResourceId>,
  pub version: Option<u32>,
}

/// The opening of `brief` a proposal shows: its first [`BRIEF_CHARS`] characters, with an ellipsis when cut.
pub fn brief_excerpt(brief: &str) -> String {
  let brief = brief.trim();
  if brief.chars().count() <= BRIEF_CHARS {
    return brief.to_owned();
  }
  let kept: String = brief.chars().take(BRIEF_CHARS - 1).collect();
  format!("{}…", kept.trim_end())
}
