//! Create & run on the new task form (02.09 `l2YA4`): check that the run can start, save the
//! task and queue it, then ask the runner to start its run. Every step is safe to retry: the
//! draft is saved under the form's batch key, queueing a queued task changes nothing, and the
//! run is started under the form's run key.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  resource::{AgentProfile, Pipeline, ResourceKind, Version},
  run::{PreflightInput, PreflightProblem, RunLimit, preflight},
  store::{ConnectionStore, ResourceStore, TaskStore},
  task::{QueueError, Task, TaskDetails, TaskStatus},
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError};

use super::data::save_draft;
use crate::{
  messages::{self, Message},
  runner::{RequestError, RunRequestError},
};

/// Why the task was not queued for a run.
#[derive(Debug)]
pub enum QueueFailure {
  Store(StoreError),
  /// The run could not start with the task's configuration; nothing was saved.
  Preflight(Vec<PreflightProblem>),
  Queue(QueueError),
}

impl From<StoreError> for QueueFailure {
  fn from(error: StoreError) -> Self {
    Self::Store(error)
  }
}

impl QueueFailure {
  pub fn messages(&self) -> Vec<Message> {
    match self {
      Self::Store(error) => vec![messages::store_error(error)],
      Self::Preflight(problems) => problems.iter().map(preflight_message).collect(),
      Self::Queue(_) => vec![Message::new("runs.start.incomplete")],
    }
  }
}

/// Checks the configuration `details` would run with, then saves the task under `batch` and
/// queues it.
pub fn queue_in(repo_root: &Path, batch: IdempotencyKey, details: TaskDetails) -> Result<Task, QueueFailure> {
  let mut db = ProjectDb::open(repo_root)?;
  let problems = check(&db, &details)?;
  if !problems.is_empty() {
    return Err(QueueFailure::Preflight(problems));
  }
  let mut task = save_draft(&mut db, batch, details, Timestamp::now())?;
  if task.status() == TaskStatus::Draft {
    task.queue().map_err(QueueFailure::Queue)?;
    db.save_task(&task)?;
  }
  Ok(task)
}

/// Problems the runner would refuse the run for, as far as the configuration shows them. The
/// runner checks again and also checks the repository.
fn check(db: &ProjectDb, details: &TaskDetails) -> Result<Vec<PreflightProblem>, StoreError> {
  let pipeline = match details.pipeline {
    Some(pipeline) => db.latest_version::<Pipeline>(pipeline)?,
    None => None,
  };
  let mut profiles: BTreeMap<ResourceId, Version<AgentProfile>> = BTreeMap::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    if let Some(version) = db.latest_version::<AgentProfile>(resource.id())? {
      profiles.insert(resource.id(), version);
    }
  }
  let connections = db.connections()?;
  Ok(preflight(PreflightInput {
    pipeline: pipeline.as_ref().map(Version::body),
    profiles: &profiles,
    connections: &connections,
  }))
}

/// What a person reads for a preflight problem.
pub fn preflight_message(problem: &PreflightProblem) -> Message {
  let step =
    |key: &'static str, step: &orchester_core::resource::StepKey| Message::new(key).with("step", step.to_string());
  match problem {
    PreflightProblem::PipelineNotPublished => Message::new("runs.preflight.pipelineNotPublished"),
    PreflightProblem::NoSteps => Message::new("runs.preflight.noSteps"),
    PreflightProblem::ToolStep { step: key } => step("runs.preflight.toolStep", key),
    PreflightProblem::ApprovalGate { step: key } => step("runs.preflight.approvalGate", key),
    PreflightProblem::ProfileNotPublished { step: key } => step("runs.preflight.profileNotPublished", key),
    PreflightProblem::ConnectionMissing { step: key } => step("runs.preflight.connectionMissing", key),
    PreflightProblem::UnsupportedProvider { step: key } => step("runs.preflight.unsupportedProvider", key),
    PreflightProblem::ConnectionNotUsable { step: key, .. } => step("runs.preflight.connectionNotUsable", key),
    PreflightProblem::NotAGitRepository => Message::new("runs.preflight.notAGitRepository"),
    PreflightProblem::NoCommits => Message::new("runs.preflight.noCommits"),
  }
}

/// What a person reads when the runner did not start the run.
pub fn start_messages(error: &RunRequestError) -> Vec<Message> {
  match error {
    RunRequestError::Request(RequestError::NotConnected) => vec![Message::new("runs.start.runnerUnavailable")],
    RunRequestError::Request(RequestError::Lost) => vec![Message::new("runs.start.lost")],
    RunRequestError::Rejected(Rejection::Preflight { problems }) => problems.iter().map(preflight_message).collect(),
    RunRequestError::Rejected(Rejection::TaskNotFound) => vec![Message::new("runs.start.taskMissing")],
    RunRequestError::Rejected(Rejection::TaskNotQueued) => vec![Message::new("runs.start.notQueued")],
    RunRequestError::Rejected(
      Rejection::Failed { .. }
      | Rejection::RunNotFound
      | Rejection::RunNotRunning
      | Rejection::RunNotPaused
      | Rejection::RunEnded
      | Rejection::LimitNotRaised { .. }
      | Rejection::LimitNotAboveUsed { .. }
      | Rejection::BudgetAboveMaximum { .. },
    ) => vec![Message::new("runs.start.failed")],
  }
}

/// What a person reads when the runner did not pause, resume or cancel a run, or raise its
/// limits.
pub fn control_message(error: &RunRequestError) -> Message {
  match error {
    RunRequestError::Request(RequestError::NotConnected) => Message::new("runs.control.runnerUnavailable"),
    RunRequestError::Request(RequestError::Lost) => Message::new("runs.control.lost"),
    RunRequestError::Rejected(Rejection::RunNotRunning) => Message::new("runs.control.notRunning"),
    RunRequestError::Rejected(Rejection::RunNotPaused) => Message::new("runs.control.notPaused"),
    RunRequestError::Rejected(Rejection::RunEnded) => Message::new("runs.control.ended"),
    RunRequestError::Rejected(Rejection::LimitNotRaised { .. }) => Message::new("runs.limit.notRaised"),
    RunRequestError::Rejected(Rejection::LimitNotAboveUsed {
      limit: RunLimit::Tokens,
    }) => Message::new("runs.limit.tokensNotAboveUsed"),
    RunRequestError::Rejected(Rejection::LimitNotAboveUsed { limit: RunLimit::Time }) => {
      Message::new("runs.limit.timeNotAboveUsed")
    }
    RunRequestError::Rejected(Rejection::BudgetAboveMaximum { max }) => {
      Message::new("tokens.errors.aboveMax").with("max", compact_tokens(max.get()))
    }
    RunRequestError::Rejected(
      Rejection::RunNotFound
      | Rejection::TaskNotFound
      | Rejection::TaskNotQueued
      | Rejection::Preflight { .. }
      | Rejection::Failed { .. },
    ) => Message::new("runs.control.failed"),
  }
}
