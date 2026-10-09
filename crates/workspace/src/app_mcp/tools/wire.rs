//! How the tools name Orchester's states: snake_case words, the same in arguments and results.

use orchester_core::{
  run::{RunStatus, StepStatus, TaskPhase},
  task::{
    TaskStatus,
    organization::{Priority, WorkflowStatus},
  },
  time::Timestamp,
};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

/// A task's workflow status (Backlog to Canceled), which a person sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Workflow {
  Backlog,
  Todo,
  InProgress,
  InReview,
  Done,
  Canceled,
}

impl From<Workflow> for WorkflowStatus {
  fn from(status: Workflow) -> Self {
    match status {
      Workflow::Backlog => Self::Backlog,
      Workflow::Todo => Self::Todo,
      Workflow::InProgress => Self::InProgress,
      Workflow::InReview => Self::InReview,
      Workflow::Done => Self::Done,
      Workflow::Canceled => Self::Canceled,
    }
  }
}

impl From<WorkflowStatus> for Workflow {
  fn from(status: WorkflowStatus) -> Self {
    match status {
      WorkflowStatus::Backlog => Self::Backlog,
      WorkflowStatus::Todo => Self::Todo,
      WorkflowStatus::InProgress => Self::InProgress,
      WorkflowStatus::InReview => Self::InReview,
      WorkflowStatus::Done => Self::Done,
      WorkflowStatus::Canceled => Self::Canceled,
    }
  }
}

/// A task's priority in the lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Rank {
  Urgent,
  High,
  Medium,
  Low,
  None,
}

impl From<Rank> for Priority {
  fn from(rank: Rank) -> Self {
    match rank {
      Rank::Urgent => Self::Urgent,
      Rank::High => Self::High,
      Rank::Medium => Self::Medium,
      Rank::Low => Self::Low,
      Rank::None => Self::None,
    }
  }
}

impl From<Priority> for Rank {
  fn from(priority: Priority) -> Self {
    match priority {
      Priority::Urgent => Self::Urgent,
      Priority::High => Self::High,
      Priority::Medium => Self::Medium,
      Priority::Low => Self::Low,
      Priority::None => Self::None,
    }
  }
}

/// Whether the task is a draft or was submitted ([`TaskStatus`]); a task whose run ended stays
/// `queued`, so where it stands is its `phase` and its `workflow_status`.
pub const fn submission(status: TaskStatus) -> &'static str {
  match status {
    TaskStatus::Draft => "draft",
    TaskStatus::Queued => "queued",
  }
}

/// Where a task stands, from its latest run.
pub const fn phase(phase: TaskPhase) -> &'static str {
  match phase {
    TaskPhase::Draft => "draft",
    TaskPhase::Queued => "queued",
    TaskPhase::Running => "running",
    TaskPhase::Waiting => "waiting",
    TaskPhase::Stuck => "stuck",
    TaskPhase::Paused => "paused",
    TaskPhase::StoppedAtLimit => "stopped_at_limit",
    TaskPhase::Interrupted => "interrupted",
    TaskPhase::Succeeded => "succeeded",
    TaskPhase::Merged => "merged",
    TaskPhase::Failed => "failed",
    TaskPhase::Cancelled => "cancelled",
  }
}

/// A run's state and, where it has one, why.
pub fn run_status(status: &RunStatus) -> Value {
  let (state, reason) = match status {
    RunStatus::Pending => ("pending", Value::Null),
    RunStatus::Running => ("running", Value::Null),
    RunStatus::Paused(reason) => ("paused", plain(reason)),
    RunStatus::WaitingForPerson(stuck) => ("waiting_for_person", plain(stuck)),
    RunStatus::Interrupted(cause) => ("interrupted", plain(cause)),
    RunStatus::Succeeded => ("succeeded", Value::Null),
    RunStatus::Failed(failure) => ("failed", plain(failure)),
    RunStatus::Cancelled => ("cancelled", Value::Null),
  };
  json!({ "state": state, "reason": reason })
}

/// A step's state and, while it waits, what for.
pub fn step_status(status: StepStatus) -> Value {
  let (state, waits_for) = match status {
    StepStatus::Pending => ("pending", Value::Null),
    StepStatus::Running => ("running", Value::Null),
    StepStatus::Waiting(reason) => ("waiting", plain(&reason)),
    StepStatus::Passed => ("passed", Value::Null),
    StepStatus::Failed => ("failed", Value::Null),
    StepStatus::Skipped => ("skipped", Value::Null),
  };
  json!({ "state": state, "waits_for": waits_for })
}

/// Milliseconds since the Unix epoch.
pub const fn time(at: Timestamp) -> i64 {
  at.unix_millis()
}

/// Plain data of core (reasons, causes), as its own serde form writes it.
fn plain(value: &impl Serialize) -> Value {
  serde_json::to_value(value).expect("run reasons are plain data with string keys")
}

/// An optional limit argument as given (with `#[serde(default)]` for one left out, `None`): `null`
/// is no limit (`Some(None)`), a number sets it.
pub fn limit<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<u64>>, D::Error> {
  Option::<u64>::deserialize(deserializer).map(Some)
}

/// A limit argument that must be given, as a number or as `null` for no limit: nothing lifts or
/// keeps a limit for a caller that left it out.
pub fn nullable<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
  Option::<u64>::deserialize(deserializer)
}

/// A limit argument as a form's field holds it: empty for no limit.
pub fn limit_text(value: Option<u64>) -> String {
  value.map(|value| value.to_string()).unwrap_or_default()
}
