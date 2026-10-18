//! Tasks before execution: drafts can be saved incomplete, but queueing requires every
//! detail a run depends on (design: "missing required details are surfaced before queueing").

use std::time::Duration;

pub mod content;
pub mod organization;
pub mod requirements;

use thiserror::Error;

use crate::{
  id::{ResourceId, TaskId},
  name::Name,
  project::TaskKey,
  resource::VersionNumber,
  sharing::TaskRevision,
  time::Timestamp,
  tokens::TokenBudget,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LimitsError {
  #[error("run time limit must be greater than zero")]
  ZeroDuration,
  #[error("run time limit can be at most {} milliseconds", crate::limits::LONGEST_TIME_LIMIT.as_millis())]
  TooLong,
}

/// Per-task token budget and time limit on active agent time; each run of the task starts with
/// them. Both are optional and have no maximum: `None` means no limit (user decision 2026-10-09).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskLimits {
  budget: Option<TokenBudget>,
  max_duration: Option<Duration>,
}

impl TaskLimits {
  /// No budget and no time limit.
  pub const NONE: Self = Self {
    budget: None,
    max_duration: None,
  };

  pub fn new(budget: Option<TokenBudget>, max_duration: Option<Duration>) -> Result<Self, LimitsError> {
    if max_duration.is_some_and(|limit| limit.is_zero()) {
      return Err(LimitsError::ZeroDuration);
    }
    if crate::limits::too_long(max_duration) {
      return Err(LimitsError::TooLong);
    }
    Ok(Self { budget, max_duration })
  }

  pub const fn budget(self) -> Option<TokenBudget> {
    self.budget
  }

  pub const fn max_duration(self) -> Option<Duration> {
    self.max_duration
  }
}

/// User-editable content of a task. Everything but the title may be missing in a draft.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDetails {
  pub title: Name,
  pub brief: String,
  pub acceptance_criteria: Vec<String>,
  pub pipeline: Option<ResourceId>,
  pub limits: TaskLimits,
}

/// A detail a task must have before it can be queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MissingDetail {
  Brief,
  AcceptanceCriteria,
  Pipeline,
}

impl TaskDetails {
  pub fn missing(&self) -> Vec<MissingDetail> {
    missing_details(&self.brief, &self.acceptance_criteria, self.pipeline.is_some())
  }
}

/// Completeness is identical for local resource IDs and shared pipeline slugs.
pub fn missing_details(brief: &str, criteria: &[String], pipeline: bool) -> Vec<MissingDetail> {
  let checks = [
    (brief.trim().is_empty(), MissingDetail::Brief),
    (
      criteria.iter().all(|criterion| criterion.trim().is_empty()),
      MissingDetail::AcceptanceCriteria,
    ),
    (!pipeline, MissingDetail::Pipeline),
  ];
  checks
    .into_iter()
    .filter_map(|(missing, detail)| missing.then_some(detail))
    .collect()
}

/// Whether a person has submitted the task for running (`tasks.status`): a draft, or queued with its
/// pipeline version pinned. It does not follow the run lifecycle: a task whose run finished stays
/// `Queued`. Where a task stands comes from its latest run ([`crate::run::TaskPhase::of`]), and the
/// task list's workflow status (In progress, In review, Done) is
/// [`organization::WorkflowStatus`], which run events and merges set (`docs/architecture/storage.md`,
/// "State machines").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskStatus {
  Draft,
  /// Submitted: its run is waiting to start, has started, or has ended.
  Queued,
}

impl TaskStatus {
  pub const fn check_editable(self, has_run: bool) -> Result<(), EditError> {
    if matches!(self, Self::Queued) && !has_run {
      return Err(EditError::QueuedWithoutRun);
    }
    Ok(())
  }

  pub fn check_edit(self, has_run: bool, missing: Vec<MissingDetail>) -> Result<(), EditError> {
    self.check_editable(has_run)?;
    if has_run && !missing.is_empty() {
      return Err(EditError::Incomplete(missing));
    }
    Ok(())
  }

  pub fn check_queue(self, missing: Vec<MissingDetail>) -> Result<(), QueueError> {
    if self != Self::Draft {
      return Err(QueueError::NotDraft(self));
    }
    if !missing.is_empty() {
      return Err(QueueError::Incomplete(missing));
    }
    Ok(())
  }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueError {
  #[error("only draft tasks can be queued; this task is {0:?}")]
  NotDraft(TaskStatus),
  #[error("task is missing {0:?}")]
  Incomplete(Vec<MissingDetail>),
}

/// Why a queued task cannot go back to the drafts.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReturnError {
  #[error("only a queued task can go back to the drafts")]
  NotQueued,
  #[error("the task has a run, so it stays queued")]
  HasRun,
}

/// Why a task's details cannot be changed. A queued task waiting for its first run stays locked;
/// once a run exists, edits must keep the task complete for future runs.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EditError {
  #[error("a queued task cannot be edited before its first run starts")]
  QueuedWithoutRun,
  #[error("a task with a run is missing {0:?}")]
  Incomplete(Vec<MissingDetail>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
  id: TaskId,
  key: TaskKey,
  details: TaskDetails,
  status: TaskStatus,
  pipeline_version: Option<VersionNumber>,
  created_at: Timestamp,
  organization: organization::TaskOrganization,
  revision: TaskRevision,
}

/// How a change of a stored task ended (`docs/architecture/server.md`, "The desktop's trait
/// boundary"). A change names the revision it was made on; when the task was changed meanwhile,
/// nothing is stored and the current task comes back for the person to compare with.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub enum TaskUpdate {
  Saved(Task),
  Conflict { current: Task },
}

impl TaskUpdate {
  /// The saved task; `None` for a conflict.
  pub fn saved(self) -> Option<Task> {
    match self {
      Self::Saved(task) => Some(task),
      Self::Conflict { .. } => None,
    }
  }
}

impl Task {
  /// A new task always starts as a draft.
  pub fn draft(id: TaskId, key: TaskKey, details: TaskDetails, created_at: Timestamp) -> Self {
    Self::from_stored(id, key, details, TaskStatus::Draft, None, created_at)
  }

  /// Rebuilds a persisted task. A queued task without a pin is a legacy row and cannot run.
  pub fn from_stored(
    id: TaskId,
    key: TaskKey,
    details: TaskDetails,
    status: TaskStatus,
    pipeline_version: Option<VersionNumber>,
    created_at: Timestamp,
  ) -> Self {
    Self {
      id,
      key,
      details,
      status,
      pipeline_version,
      created_at,
      organization: organization::TaskOrganization::default(),
      revision: TaskRevision::FIRST,
    }
  }

  /// The revision the task was read at; every stored change raises it.
  pub const fn revision(&self) -> TaskRevision {
    self.revision
  }

  /// Attaches the revision the task was stored with.
  #[must_use]
  pub const fn with_revision(mut self, revision: TaskRevision) -> Self {
    self.revision = revision;
    self
  }

  pub const fn id(&self) -> TaskId {
    self.id
  }

  pub fn organization(&self) -> &organization::TaskOrganization {
    &self.organization
  }

  /// Attaches persisted user metadata; run prompt details remain separate.
  pub fn with_organization(mut self, value: organization::TaskOrganization) -> Self {
    self.organization = value;
    self
  }

  pub const fn key(&self) -> &TaskKey {
    &self.key
  }

  pub const fn details(&self) -> &TaskDetails {
    &self.details
  }

  pub const fn status(&self) -> TaskStatus {
    self.status
  }

  pub const fn pipeline_version(&self) -> Option<VersionNumber> {
    self.pipeline_version
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  /// A queued task is editable once its first run is recorded.
  pub const fn check_editable(&self, has_run: bool) -> Result<(), EditError> {
    self.status.check_editable(has_run)
  }

  /// Replaces the task's details, keeping its identity and status. A draft may be incomplete;
  /// a task that has started must remain complete for its next run.
  pub fn edit(&mut self, details: TaskDetails, has_run: bool) -> Result<(), EditError> {
    self.status.check_edit(has_run, details.missing())?;
    self.details = details;
    Ok(())
  }

  /// Moves a complete draft to the queue; an incomplete one reports what is missing.
  pub fn queue(&mut self, pipeline_version: VersionNumber) -> Result<(), QueueError> {
    self.status.check_queue(self.details.missing())?;
    self.status = TaskStatus::Queued;
    self.pipeline_version = Some(pipeline_version);
    Ok(())
  }

  /// Takes a queued task that never started back to the drafts, e.g. one a held group released.
  pub fn return_to_draft(&mut self, has_run: bool) -> Result<(), ReturnError> {
    if self.status != TaskStatus::Queued {
      return Err(ReturnError::NotQueued);
    }
    if has_run {
      return Err(ReturnError::HasRun);
    }
    self.status = TaskStatus::Draft;
    self.pipeline_version = None;
    Ok(())
  }
}
