//! Applying the project stream's task deltas to the cache (`docs/architecture/server.md`, "Events", "Applying"): a
//! `task.changed` made on the cached revision moves the task to its own; one made on another revision means the cache
//! missed a change, so the caller reads the task again. A delta not newer than the cache, the person's own change
//! among them, changes nothing.

use std::{cmp::Ordering, num::NonZeroU32};

use super::{CachedClaim, CachedRunStatus, CachedTask, SharedPerson, TaskRevision};
use crate::{
  id::{ModuleId, TagId},
  name::Name,
  run::TaskPhase,
  task::{
    TaskLimits, TaskStatus,
    organization::{BlockingTask, OrganizationError, Priority, TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
};

/// One task's delta: who made it, when, and what it changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDelta {
  pub by: SharedPerson,
  pub at: Timestamp,
  pub change: TaskDeltaKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskDeltaKind {
  /// `task.changed`.
  Changed(TaskChange),
  /// `task.claim`: the claim as it is now.
  Claim(Option<CachedClaim>),
  /// `task.run`.
  Run(RunProgress),
}

/// `task.changed`: made on `base`, now at `revision`, with only the fields that changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskChange {
  pub revision: TaskRevision,
  pub base: TaskRevision,
  pub title: Option<Name>,
  pub brief: Option<String>,
  pub criteria: Option<Vec<String>>,
  pub pipeline: Option<Option<String>>,
  pub limits: Option<TaskLimits>,
  pub status: Option<TaskStatus>,
  pub module: Option<Option<ModuleId>>,
  pub priority: Option<Priority>,
  pub workflow: Option<WorkflowStatus>,
  pub tags_added: Vec<TagId>,
  pub tags_removed: Vec<TagId>,
  /// The task's blocking links as they are now.
  pub blocked_by: Option<Vec<BlockingTask>>,
  pub blocked_reason: Option<Option<String>>,
}

impl TaskChange {
  /// A change made on `base` that moves the task to `revision` and changes no field yet.
  pub const fn new(base: TaskRevision, revision: TaskRevision) -> Self {
    Self {
      revision,
      base,
      title: None,
      brief: None,
      criteria: None,
      pipeline: None,
      limits: None,
      status: None,
      module: None,
      priority: None,
      workflow: None,
      tags_added: Vec::new(),
      tags_removed: Vec::new(),
      blocked_by: None,
      blocked_reason: None,
    }
  }
}

/// `task.run`: a run's progress, light. The rest of the run is read with the task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunProgress {
  pub number: NonZeroU32,
  pub phase: TaskPhase,
  /// The step as the cache names it.
  pub step: Option<String>,
  pub phase_since: Timestamp,
  pub updated_at: Timestamp,
}

/// What a delta does to a cached task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeltaOutcome {
  /// The task as the delta leaves it.
  Applied(Box<CachedTask>),
  /// The cache has it already: the person's own change, or one not newer than the cache.
  AlreadyHave,
  /// The cache cannot take it (a `task.changed` made on another revision, a run it does not know): read the task
  /// again and take it as the confirmed state.
  Stale,
}

impl CachedTask {
  /// This task after `delta`, which is one of this task's.
  pub fn apply(&self, delta: &TaskDelta) -> DeltaOutcome {
    match &delta.change {
      TaskDeltaKind::Changed(change) => self.changed(change, &delta.by, delta.at),
      TaskDeltaKind::Claim(claim) => self.claimed(claim.as_ref()),
      TaskDeltaKind::Run(progress) => self.progressed(progress),
    }
  }

  fn changed(&self, change: &TaskChange, by: &SharedPerson, at: Timestamp) -> DeltaOutcome {
    if change.revision.get() <= self.revision.get() {
      return DeltaOutcome::AlreadyHave;
    }
    if change.base != self.revision {
      return DeltaOutcome::Stale;
    }
    let Ok(organization) = organized(&self.organization, change) else {
      // The server never confirms such an organization; what it has is read with the task.
      return DeltaOutcome::Stale;
    };
    let mut task = self.clone();
    task.revision = change.revision;
    task.organization = organization;
    task.updated_by.clone_from(&by.name);
    task.updated_at = at;
    if let Some(title) = &change.title {
      task.title = title.clone();
    }
    if let Some(brief) = &change.brief {
      task.brief.clone_from(brief);
    }
    if let Some(criteria) = &change.criteria {
      task.criteria.clone_from(criteria);
    }
    if let Some(pipeline) = &change.pipeline {
      task.pipeline.clone_from(pipeline);
    }
    task.limits = change.limits.unwrap_or(task.limits);
    task.status = change.status.unwrap_or(task.status);
    task.module = change.module.unwrap_or(task.module);
    DeltaOutcome::Applied(Box::new(task))
  }

  fn claimed(&self, claim: Option<&CachedClaim>) -> DeltaOutcome {
    if self.claim.as_ref() == claim {
      return DeltaOutcome::AlreadyHave;
    }
    let mut task = self.clone();
    task.claim = claim.cloned();
    DeltaOutcome::Applied(Box::new(task))
  }

  /// A run the cache does not have yet is read with the task: the delta lacks its holder and start.
  fn progressed(&self, progress: &RunProgress) -> DeltaOutcome {
    let Some(run) = &self.run else {
      return DeltaOutcome::Stale;
    };
    match progress.number.cmp(&run.number) {
      Ordering::Less => return DeltaOutcome::AlreadyHave,
      Ordering::Greater => return DeltaOutcome::Stale,
      Ordering::Equal if progress.updated_at <= run.updated_at => return DeltaOutcome::AlreadyHave,
      Ordering::Equal => {}
    }
    let run = CachedRunStatus {
      phase: progress.phase,
      step: progress.step.clone(),
      phase_since: progress.phase_since,
      updated_at: progress.updated_at,
      ..run.clone()
    };
    DeltaOutcome::Applied(Box::new(CachedTask {
      run: Some(run),
      ..self.clone()
    }))
  }
}

fn organized(current: &TaskOrganization, change: &TaskChange) -> Result<TaskOrganization, OrganizationError> {
  let tags = current
    .clone()
    .remove_tags(&change.tags_removed)
    .add_tags(&change.tags_added)
    .tags()
    .to_vec();
  let reason = match &change.blocked_reason {
    Some(reason) => reason.clone(),
    None => current.blocked_reason().map(str::to_owned),
  };
  let links = match &change.blocked_by {
    Some(links) => links.clone(),
    None => current.dependencies().to_vec(),
  };
  Ok(
    TaskOrganization::new(
      change.workflow.unwrap_or(current.status()),
      change.priority.unwrap_or(current.priority()),
      reason,
      links,
      tags,
    )?
    .with_no_automatic_merge(current.no_automatic_merge())
    .with_delivers(current.delivers()),
  )
}

/// `tag.deleted` and `tag.merged`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagRemap {
  Deleted(TagId),
  Merged { from: TagId, into: TagId },
}

/// The `tasks` that `remap` changes, with their tags remapped: a deleted tag is removed, a merged one becomes the tag
/// it went into. Their revisions stay: each rewritten task's own `task.changed` follows on the stream.
pub fn remap_tags<'a>(tasks: impl IntoIterator<Item = &'a CachedTask>, remap: TagRemap) -> Vec<CachedTask> {
  let gone = match remap {
    TagRemap::Deleted(tag) | TagRemap::Merged { from: tag, .. } => tag,
  };
  tasks
    .into_iter()
    .filter(|task| task.organization.tags().contains(&gone))
    .map(|task| {
      let mut organization = task.organization.clone().remove_tags(&[gone]);
      if let TagRemap::Merged { into, .. } = remap {
        organization = organization.add_tags(&[into]);
      }
      CachedTask {
        organization,
        ..task.clone()
      }
    })
    .collect()
}
