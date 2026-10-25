//! What a run of a pipeline delivers (ORC-93): repository changes, which are merged, or a result that is no commit —
//! the last step's report, or work done through MCP tools outside the repository. A task may say otherwise for its
//! own runs ([`Deliverable::for_task`]). Pipelines saved before this existed deliver changes.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// What a run delivers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deliverable {
  /// Repository changes, merged into the base branch.
  #[default]
  Changes,
  /// The last step's report; no repository changes are expected.
  Report,
  /// Work done through MCP tools outside the repository (a design, tickets, data in a service): the last step's report
  /// and the record of the tool calls that changed something.
  Outside,
  /// Repository changes when the run made any, else the last step's report.
  ChangesOrReport,
}

/// What a run that passed every step and committed nothing waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WithoutCommits {
  /// Its result is its report: a person completes the task, or the Orchestrator does as it would merge.
  Completes,
  /// Its pipeline expects changes: a person decides how it goes on, never the Orchestrator.
  Flagged,
}

impl Deliverable {
  pub const ALL: [Self; 4] = [Self::Changes, Self::Report, Self::Outside, Self::ChangesOrReport];

  /// How it is written in a pipeline's body and named by Orchester's MCP tools.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Changes => "changes",
      Self::Report => "report",
      Self::Outside => "outside",
      Self::ChangesOrReport => "changes_or_report",
    }
  }

  /// Whether it is [`Self::Changes`], which a pipeline's body leaves out.
  pub const fn is_changes(&self) -> bool {
    matches!(self, Self::Changes)
  }

  /// What a run that passed every step and committed nothing waits for.
  pub const fn without_commits(self) -> WithoutCommits {
    match self {
      Self::Changes => WithoutCommits::Flagged,
      Self::Report | Self::Outside | Self::ChangesOrReport => WithoutCommits::Completes,
    }
  }

  /// What a task's runs deliver: the task's own `choice`, else what its `pipeline` delivers.
  pub fn for_task(choice: Option<Self>, pipeline: Self) -> Self {
    choice.unwrap_or(pipeline)
  }
}

impl fmt::Display for Deliverable {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(self.as_str())
  }
}

/// A name that is none of [`Deliverable::ALL`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not what a run delivers: changes, report, outside or changes_or_report")]
pub struct UnknownDeliverable(String);

impl FromStr for Deliverable {
  type Err = UnknownDeliverable;

  fn from_str(value: &str) -> Result<Self, UnknownDeliverable> {
    Self::ALL
      .into_iter()
      .find(|deliverable| deliverable.as_str() == value)
      .ok_or_else(|| UnknownDeliverable(value.to_owned()))
  }
}
