//! Between the server's tasks and the project database's (`docs/architecture/server.md`, "API",
//! "Pipelines by name"): a task the server confirmed becomes a cached task; a local task is uploaded
//! with its pipeline's slug and its tags by name; and a change sends only the fields that differ
//! from the cached task, so a person who lacks a task's pipeline does not clear it by editing its
//! title.

use std::{num::NonZeroU32, time::Duration};

use orchester_api::{
  Organization, RunStatus, SharedClaim, SharedLimits, SharedTask, TaskChanges, TaskContent, UserRef,
};
use orchester_core::{
  id::ResourceId,
  name::Name,
  store::ResourceStore,
  task::{Task, TaskDetails, TaskLimits, TaskStatus, organization::TaskOrganization},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::{CachedClaim, CachedRunStatus, CachedTask, ProjectDb, SharedPerson, StoreError};

use crate::sharing::SharingError;

/// `task` as the server confirmed it, for the cache.
pub fn cached(task: &SharedTask) -> Result<CachedTask, SharingError> {
  let content = &task.content;
  Ok(CachedTask {
    id: task.id,
    number: NonZeroU32::new(task.number).ok_or(SharingError::InvalidAnswer("task number"))?,
    revision: task.revision,
    title: Name::new(&content.title).map_err(|_| SharingError::InvalidAnswer("task title"))?,
    brief: content.brief.clone(),
    criteria: content.criteria.clone(),
    pipeline: content.pipeline.clone(),
    limits: content.limits.as_ref().map_or(Ok(TaskLimits::NONE), limits)?,
    status: task.status,
    organization: content
      .organization()
      .core()
      .map_err(|_| SharingError::InvalidAnswer("task organization"))?,
    former_key: task.former_key.clone(),
    created_by: task.created.by.display_name.clone(),
    created_at: Timestamp::from_unix_millis(task.created.at),
    updated_by: task.updated.by.display_name.clone(),
    updated_at: Timestamp::from_unix_millis(task.updated.at),
    claim: task.claim.as_ref().map(claim),
    run: task.run.as_ref().map(run_status).transpose()?,
  })
}

pub fn person(user: &UserRef) -> SharedPerson {
  SharedPerson {
    id: user.id,
    name: user.display_name.clone(),
  }
}

pub fn claim(claim: &SharedClaim) -> CachedClaim {
  CachedClaim {
    id: claim.id,
    holder: person(&claim.holder),
    kind: claim.kind,
    since: Timestamp::from_unix_millis(claim.since),
    heartbeat_at: Timestamp::from_unix_millis(claim.heartbeat_at),
    stale: claim.stale,
    taken_over_from: claim.taken_over_from.as_ref().map(person),
  }
}

fn run_status(run: &RunStatus) -> Result<CachedRunStatus, SharingError> {
  Ok(CachedRunStatus {
    holder: person(&run.holder),
    number: NonZeroU32::new(run.number).ok_or(SharingError::InvalidAnswer("run number"))?,
    phase: run.phase,
    step: run.step.clone(),
    started_at: Timestamp::from_unix_millis(run.started_at),
    phase_since: Timestamp::from_unix_millis(run.phase_since),
    updated_at: Timestamp::from_unix_millis(run.updated_at),
  })
}

fn limits(limits: &SharedLimits) -> Result<TaskLimits, SharingError> {
  let budget = limits
    .budget_tokens
    .map(TokenBudget::new)
    .transpose()
    .map_err(|_| SharingError::InvalidAnswer("task budget"))?;
  TaskLimits::new(budget, limits.max_duration_ms.map(Duration::from_millis))
    .map_err(|_| SharingError::InvalidAnswer("task time limit"))
}

/// A task's limits as the server keeps them: `None` for a task without any.
pub fn shared_limits(limits: TaskLimits) -> Option<SharedLimits> {
  (limits != TaskLimits::NONE).then(|| SharedLimits {
    budget_tokens: limits.budget().map(TokenBudget::get),
    // Beyond u64 milliseconds is beyond any limit the store keeps (i64 milliseconds); the longest
    // limit the server takes stands for it.
    max_duration_ms: limits
      .max_duration()
      .map(|limit| u64::try_from(limit.as_millis()).unwrap_or(u64::MAX)),
  })
}

/// The slug of this computer's pipeline `id`: pipelines are shared by slug.
pub fn pipeline_slug(db: &ProjectDb, id: Option<ResourceId>) -> Result<Option<String>, StoreError> {
  id.map(|id| {
    db.resource(id)?
      .map(|resource| resource.slug().as_str().to_owned())
      .ok_or(StoreError::ResourceNotFound(id))
  })
  .transpose()
}

/// A new task's content: `details` and its `organization`.
pub fn content(
  db: &ProjectDb,
  details: &TaskDetails,
  organization: &TaskOrganization,
) -> Result<TaskContent, StoreError> {
  let mut content = TaskContent {
    title: details.title.as_str().to_owned(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: pipeline_slug(db, details.pipeline)?,
    limits: shared_limits(details.limits),
    workflow: organization.status(),
    priority: organization.priority(),
    tags: Vec::new(),
    blocking: Vec::new(),
    blocked_reason: None,
  };
  content.set_organization(organization);
  Ok(content)
}

/// The changes from `cached` (the task as the cache has it) to `details` and `status`: only the
/// fields that differ.
pub fn content_changes(
  db: &ProjectDb,
  cached: &Task,
  after: &TaskDetails,
  status: TaskStatus,
) -> Result<TaskChanges, StoreError> {
  let before = cached.details();
  let mut changes = TaskChanges::default();
  if before.title != after.title {
    changes.title = Some(after.title.as_str().to_owned());
  }
  if before.brief != after.brief {
    changes.brief = Some(after.brief.clone());
  }
  if before.acceptance_criteria != after.acceptance_criteria {
    changes.criteria = Some(after.acceptance_criteria.clone());
  }
  if before.pipeline != after.pipeline {
    changes.pipeline = Some(pipeline_slug(db, after.pipeline)?);
  }
  if before.limits != after.limits {
    changes.limits = Some(shared_limits(after.limits));
  }
  if cached.status() != status {
    changes.status = Some(status);
  }
  Ok(changes)
}

/// `changes` with the organization fields that differ from `before` to `after`.
pub fn with_organization(mut changes: TaskChanges, before: &TaskOrganization, after: &TaskOrganization) -> TaskChanges {
  let (before, after) = (Organization::from_core(before), Organization::from_core(after));
  if before.workflow != after.workflow {
    changes.workflow = Some(after.workflow);
  }
  if before.priority != after.priority {
    changes.priority = Some(after.priority);
  }
  if before.tags != after.tags {
    changes.tags = Some(after.tags);
  }
  if before.blocking != after.blocking {
    changes.blocking = Some(after.blocking);
  }
  if before.blocked_reason != after.blocked_reason {
    changes.blocked_reason = Some(after.blocked_reason);
  }
  changes
}

/// Whether `changes` change nothing.
pub fn is_empty(changes: &TaskChanges) -> bool {
  changes.title.is_none()
    && changes.brief.is_none()
    && changes.criteria.is_none()
    && changes.pipeline.is_none()
    && changes.limits.is_none()
    && changes.status.is_none()
    && changes.workflow.is_none()
    && changes.priority.is_none()
    && changes.tags.is_none()
    && changes.blocking.is_none()
    && changes.blocked_reason.is_none()
}
