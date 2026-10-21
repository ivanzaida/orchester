//! How the tools name Orchester's states: snake_case words, the same in arguments and results.

use orchester_core::{
  id::ProjectId,
  project::TaskKey,
  run::{InterruptCause, RunStatus, RunnerFailure, StepStatus, TaskPhase},
  task::{
    TaskStatus,
    estimate::StoryEstimate,
    organization::{Priority, WorkflowStatus},
  },
  time::Timestamp,
};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

use crate::links::{Link, TaskPage};

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

/// A task's story points, who set them (`person` or `automation`) and automation's reason; `null` while it has
/// none.
pub fn story_points(estimate: Option<&StoryEstimate>) -> Value {
  estimate.map_or(Value::Null, |estimate| {
    json!({
      "points": estimate.points().get(),
      "set_by": plain(&estimate.source()),
      "reason": estimate.reason(),
    })
  })
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

/// A run's state and, where it has one, why; for one the runner interrupted after its own failure, also what kind of
/// failure (`runner_failure`: `storage_contention`, `storage_failure` or `other`; `null` when it was not recorded),
/// `runner_failure` being what the store read for the run.
pub fn run_status(status: &RunStatus, runner_failure: Option<RunnerFailure>) -> Value {
  let (state, reason) = match status {
    RunStatus::Pending => ("pending", Value::Null),
    RunStatus::Running => ("running", Value::Null),
    RunStatus::Paused(reason) => ("paused", plain(reason)),
    RunStatus::WaitingForPerson(stuck) => ("waiting_for_person", plain(stuck)),
    RunStatus::Interrupted(InterruptCause::RunnerFailed) => {
      return json!({
        "state": "interrupted",
        "reason": plain(&InterruptCause::RunnerFailed),
        "runner_failure": runner_failure.as_ref().map(plain),
      });
    }
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

/// The link to the page of task `key` of `project`, on `page`. A key the app stored always reads as one; one
/// that does not gives no link rather than a wrong one, and the item still names its task.
pub fn task_link(project: ProjectId, key: &str, page: TaskPage) -> Option<String> {
  key
    .parse::<TaskKey>()
    .ok()
    .map(|key| Link::Task(project, key, page).path())
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

/// An optional text argument as given (with `#[serde(default)]` for one left out, `None`): `null`
/// clears it (`Some(None)`), a text sets it.
pub fn clearable<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
  Option::<String>::deserialize(deserializer).map(Some)
}

/// A limit argument as a form's field holds it: empty for no limit.
pub fn limit_text(value: Option<u64>) -> String {
  value.map(|value| value.to_string()).unwrap_or_default()
}
