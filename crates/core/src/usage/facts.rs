//! What a project's store gives the Usage dashboard: the usage each completed turn reported with
//! when it completed, and the invocations and runs the turns belong to. Nothing here is estimated;
//! what the store does not know stays `None`.

use std::sync::Arc;

use crate::{
  connection::AgentProvider,
  id::{AgentAccountId, InvocationId, ProjectId, ResourceId, RunId, TaskId},
  run::{EditProgress, RunNumber, TokenUsage},
  time::Timestamp,
};

/// The usage records of one project from a time on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsageFacts {
  pub turns: Vec<TurnUsage>,
  /// The reviews of auto mode's reviewer whose usage is known.
  pub reviews: Vec<ReviewUsage>,
  pub invocations: Vec<InvocationFacts>,
  pub runs: Vec<RunFacts>,
  /// When the project's latest turn completed, at any time: the no-runs state names it.
  pub last_turn: Option<Timestamp>,
}

/// One completed turn's usage, as its CLI reported it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurnUsage {
  pub at: Timestamp,
  pub invocation: InvocationId,
  pub usage: TokenUsage,
}

/// What one review of auto mode's reviewer reported it spent, with the invocation whose request it
/// judged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewUsage {
  /// When the review ended.
  pub at: Timestamp,
  pub invocation: InvocationId,
  /// The reviewer's provider.
  pub cli: AgentProvider,
  /// The reviewer's model.
  pub model: Arc<str>,
  pub usage: TokenUsage,
}

/// Whether an agent invocation got the code index, as it recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IndexGiven {
  Yes,
  No,
  /// Recorded before what an invocation got of the index was kept.
  Unknown,
}

/// One launch of an agent CLI or a command tool, with what it ran on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationFacts {
  pub id: InvocationId,
  pub run: RunId,
  /// The provider of the profile's connection; `None` for a command tool, or when the connection
  /// no longer exists.
  pub cli: Option<AgentProvider>,
  /// `None` for a command tool.
  pub model: Option<Arc<str>>,
  /// The agent profile's name; `None` for a command tool.
  pub profile: Option<Arc<str>>,
  /// The agent profile's resource; `None` for a command tool.
  pub profile_id: Option<ResourceId>,
  /// The CLI account it started on; `None` for a command tool, a native agent, or one started before
  /// accounts were recorded.
  pub account: Option<AgentAccountId>,
  /// The step's name in the run's pinned pipeline (*Implement*).
  pub step: Arc<str>,
  pub started_at: Timestamp,
  /// `None` while it runs.
  pub ended_at: Option<Timestamp>,
  /// Active agent time when it was read, waits for a person left out.
  pub active_ms: u64,
  /// What it used before its first edit; `None` for a command tool or one recorded before it was kept.
  pub edit: Option<EditProgress>,
  pub index: IndexGiven,
}

/// How a run stands, as the Overview and the task list name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RunOutcome {
  Pending,
  Running,
  Paused,
  Waiting,
  Interrupted,
  Succeeded,
  Failed,
  Cancelled,
}

impl RunOutcome {
  /// A run whose latest attempt failed, as the Overview's *Failed* filter counts it.
  pub const fn failed(self) -> bool {
    matches!(self, Self::Failed)
  }

  /// A run that ended without failing.
  pub const fn finished(self) -> bool {
    matches!(self, Self::Succeeded | Self::Cancelled)
  }
}

/// One run and the task it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFacts {
  pub id: RunId,
  pub number: RunNumber,
  pub task: TaskId,
  /// `ORC-128`.
  pub task_key: Arc<str>,
  pub task_title: Arc<str>,
  /// The pinned pipeline's name; `None` when its resource is gone.
  pub pipeline: Option<Arc<str>>,
  /// The pinned pipeline's resource; `None` when it is gone.
  pub pipeline_id: Option<ResourceId>,
  /// The task's tags by name.
  pub tags: Vec<Arc<str>>,
  pub outcome: RunOutcome,
}

/// One project's facts with what names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFacts {
  pub project: ProjectId,
  pub name: Arc<str>,
  pub facts: UsageFacts,
}
