//! User-owned task organization, independent of execution and scheduling.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  id::{RunId, TagId, TaskId},
  name::Name,
  time::Timestamp,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WorkflowStatus {
  #[default]
  Backlog,
  Todo,
  InProgress,
  InReview,
  Done,
  Canceled,
}

impl WorkflowStatus {
  pub const ALL: [Self; 6] = [
    Self::Backlog,
    Self::Todo,
    Self::InProgress,
    Self::InReview,
    Self::Done,
    Self::Canceled,
  ];

  /// Cancellation cannot commit until the existing run cancellation path has finished.
  pub fn check_change(self, active: Option<RunId>) -> Result<(), OrganizationError> {
    if !self.can_change(active.is_some())
      && let Some(run) = active
    {
      return Err(OrganizationError::StopRunFirst(run));
    }
    Ok(())
  }

  /// Shared run reports carry a phase, while local runs carry their RunId.
  pub fn can_change(self, run_not_ended: bool) -> bool {
    self != Self::Canceled || !run_not_ended
  }

  /// Done or Canceled: a person closed the task, or a merge finished it, until it is reopened.
  pub const fn is_closed(self) -> bool {
    matches!(self, Self::Done | Self::Canceled)
  }
}

/// Ordering is for task lists only, never runner scheduling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
  Urgent,
  High,
  Medium,
  Low,
  #[default]
  None,
}

impl Priority {
  pub const ALL: [Self; 5] = [Self::Urgent, Self::High, Self::Medium, Self::Low, Self::None];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyGate {
  Review,
  Merge,
}

impl DependencyGate {
  pub const fn is_completed(self, phase: crate::run::TaskPhase) -> bool {
    match self {
      Self::Review => matches!(phase, crate::run::TaskPhase::Succeeded | crate::run::TaskPhase::Merged),
      Self::Merge => matches!(phase, crate::run::TaskPhase::Merged),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockingTask {
  pub task: TaskId,
  pub gate: DependencyGate,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StoredOrganization", into = "StoredOrganization")]
pub struct TaskOrganization {
  status: WorkflowStatus,
  priority: Priority,
  blocked_reason: Option<String>,
  dependencies: Vec<BlockingTask>,
  tags: Vec<TagId>,
  /// *Don't merge automatically* (ORC-69): the Orchestrator never merges a run of the task on its own; the turn a
  /// run's success starts only tells the person.
  no_automatic_merge: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct StoredOrganization {
  status: WorkflowStatus,
  priority: Priority,
  blocked_reason: Option<String>,
  dependencies: Vec<BlockingTask>,
  tags: Vec<TagId>,
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  no_automatic_merge: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OrganizationError {
  #[error("a blocked task requires a reason")]
  MissingReason,
  #[error("task dependency would form a cycle")]
  DependencyCycle,
  #[error("stop run {0} before canceling the task")]
  StopRunFirst(RunId),
  #[error("tag name already exists")]
  DuplicateTag,
  #[error("tag {0} does not exist")]
  TagNotFound(TagId),
  #[error("a tag cannot merge into itself")]
  SameTag,
  #[error("task has unfinished blocking tasks")]
  Blocked(Vec<TaskId>),
}

impl TaskOrganization {
  pub fn new(
    status: WorkflowStatus,
    priority: Priority,
    blocked_reason: Option<String>,
    dependencies: Vec<BlockingTask>,
    mut tags: Vec<TagId>,
  ) -> Result<Self, OrganizationError> {
    let blocked_reason = blocked_reason.map(|reason| reason.trim().to_owned());
    if blocked_reason.as_ref().is_some_and(String::is_empty) {
      return Err(OrganizationError::MissingReason);
    }
    tags.sort();
    tags.dedup();
    Ok(Self {
      status,
      priority,
      blocked_reason,
      dependencies: dependencies
        .into_iter()
        .map(|link| (link.task, link.gate))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_iter()
        .map(|(task, gate)| BlockingTask { task, gate })
        .collect(),
      tags,
      no_automatic_merge: false,
    })
  }

  pub const fn status(&self) -> WorkflowStatus {
    self.status
  }
  pub const fn priority(&self) -> Priority {
    self.priority
  }
  pub fn blocked_reason(&self) -> Option<&str> {
    self.blocked_reason.as_deref()
  }
  pub fn dependencies(&self) -> &[BlockingTask] {
    &self.dependencies
  }
  pub fn tags(&self) -> &[TagId] {
    &self.tags
  }
  /// Whether the task says *Don't merge automatically*: the Orchestrator never merges its runs on its own.
  pub const fn no_automatic_merge(&self) -> bool {
    self.no_automatic_merge
  }

  pub fn check_tags(&self, known: &std::collections::HashSet<TagId>) -> Result<(), OrganizationError> {
    if let Some(id) = self.tags.iter().find(|id| !known.contains(id)) {
      return Err(OrganizationError::TagNotFound(*id));
    }
    Ok(())
  }

  pub fn with_status(mut self, status: WorkflowStatus) -> Self {
    self.status = status;
    self
  }
  pub fn with_priority(mut self, priority: Priority) -> Self {
    self.priority = priority;
    self
  }
  pub fn with_tags(mut self, mut tags: Vec<TagId>) -> Self {
    tags.sort();
    tags.dedup();
    self.tags = tags;
    self
  }
  #[must_use]
  pub const fn with_no_automatic_merge(mut self, no_automatic_merge: bool) -> Self {
    self.no_automatic_merge = no_automatic_merge;
    self
  }

  pub fn add_tags(self, tags: &[TagId]) -> Self {
    let mut assigned = self.tags.clone();
    assigned.extend_from_slice(tags);
    self.with_tags(assigned)
  }

  pub fn remove_tags(self, tags: &[TagId]) -> Self {
    let assigned = self.tags.iter().copied().filter(|id| !tags.contains(id)).collect();
    self.with_tags(assigned)
  }

  /// Without its link to `task`, e.g. once `task` was deleted: nothing blocks it on that task any more.
  #[must_use]
  pub fn without_blocking(mut self, task: TaskId) -> Self {
    self.dependencies.retain(|link| link.task != task);
    self
  }

  pub fn check_dependencies(
    &self,
    task: TaskId,
    graph: &std::collections::HashMap<TaskId, TaskOrganization>,
  ) -> Result<(), OrganizationError> {
    let mut reached = std::collections::HashSet::new();
    let mut pending: Vec<_> = self.dependencies.iter().map(|link| link.task).collect();
    while let Some(id) = pending.pop() {
      if id == task {
        return Err(OrganizationError::DependencyCycle);
      }
      if reached.insert(id)
        && let Some(other) = graph.get(&id)
      {
        pending.extend(other.dependencies.iter().map(|link| link.task));
      }
    }
    Ok(())
  }

  /// `current` with the changes a person made from `base` to `self` in a form, so what changed
  /// meanwhile elsewhere (Orchester moving the task to In review, a bulk tag) is kept: each field
  /// the person did not change keeps its current value, and tags and links are merged by what was
  /// added and removed.
  #[must_use]
  pub fn rebase(&self, base: &Self, current: &Self) -> Self {
    fn pick<T: PartialEq + Clone>(edited: &T, base: &T, current: &T) -> T {
      if edited == base { current } else { edited }.clone()
    }
    let removed_tags: Vec<TagId> = base
      .tags
      .iter()
      .copied()
      .filter(|tag| !self.tags.contains(tag))
      .collect();
    let added_tags: Vec<TagId> = self
      .tags
      .iter()
      .copied()
      .filter(|tag| !base.tags.contains(tag))
      .collect();
    // A link the person removed goes; one the person added (or gave another gate) replaces
    // the current link to the same task.
    let added: Vec<&BlockingTask> = self
      .dependencies
      .iter()
      .filter(|link| !base.dependencies.contains(link))
      .collect();
    let mut dependencies: Vec<BlockingTask> = current
      .dependencies
      .iter()
      .filter(|link| self.dependencies.contains(link) || !base.dependencies.contains(link))
      .filter(|link| !added.iter().any(|new| new.task == link.task))
      .cloned()
      .collect();
    dependencies.extend(added.into_iter().cloned());
    Self {
      status: pick(&self.status, &base.status, &current.status),
      priority: pick(&self.priority, &base.priority, &current.priority),
      blocked_reason: pick(&self.blocked_reason, &base.blocked_reason, &current.blocked_reason),
      dependencies,
      tags: Vec::new(),
      no_automatic_merge: pick(
        &self.no_automatic_merge,
        &base.no_automatic_merge,
        &current.no_automatic_merge,
      ),
    }
    .with_tags(current.clone().remove_tags(&removed_tags).add_tags(&added_tags).tags)
  }

  /// The manual flag is informational and remains visible after links resolve.
  pub fn unfinished(&self, completed: impl Fn(&BlockingTask) -> bool) -> Vec<TaskId> {
    self
      .dependencies
      .iter()
      .filter(|link| !completed(link))
      .map(|link| link.task)
      .collect()
  }
}

impl TryFrom<StoredOrganization> for TaskOrganization {
  type Error = OrganizationError;
  fn try_from(value: StoredOrganization) -> Result<Self, Self::Error> {
    Ok(
      Self::new(
        value.status,
        value.priority,
        value.blocked_reason,
        value.dependencies,
        value.tags,
      )?
      .with_no_automatic_merge(value.no_automatic_merge),
    )
  }
}

impl From<TaskOrganization> for StoredOrganization {
  fn from(value: TaskOrganization) -> Self {
    Self {
      status: value.status,
      priority: value.priority,
      blocked_reason: value.blocked_reason,
      dependencies: value.dependencies,
      tags: value.tags,
      no_automatic_merge: value.no_automatic_merge,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
  id: TagId,
  name: Name,
}

impl Tag {
  pub const fn new(id: TagId, name: Name) -> Self {
    Self { id, name }
  }
  pub const fn id(&self) -> TagId {
    self.id
  }
  pub const fn name(&self) -> &Name {
    &self.name
  }
  pub fn check_merge(&self, into: TagId) -> Result<(), OrganizationError> {
    if self.id == into {
      return Err(OrganizationError::SameTag);
    }
    Ok(())
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeActor {
  Person,
  Orchester,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrganizationChange {
  pub actor: ChangeActor,
  pub at: Timestamp,
  pub from: TaskOrganization,
  pub to: TaskOrganization,
  pub from_tag_names: Vec<Name>,
  pub to_tag_names: Vec<Name>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskSort {
  Status,
  Priority,
}

impl TaskSort {
  pub fn compare(
    self,
    left: (&TaskOrganization, Timestamp, TaskId),
    right: (&TaskOrganization, Timestamp, TaskId),
  ) -> std::cmp::Ordering {
    let order = match self {
      Self::Status => left.0.status().cmp(&right.0.status()),
      Self::Priority => left.0.priority().cmp(&right.0.priority()),
    };
    order
      .then_with(|| right.1.cmp(&left.1))
      .then_with(|| left.2.cmp(&right.2))
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskFilter {
  pub statuses: Vec<WorkflowStatus>,
  pub priorities: Vec<Priority>,
  pub tags: Vec<TagId>,
  pub all_tags: bool,
}

impl TaskFilter {
  pub fn includes(&self, value: &TaskOrganization) -> bool {
    (self.statuses.is_empty() || self.statuses.contains(&value.status))
      && (self.priorities.is_empty() || self.priorities.contains(&value.priority))
      && (self.tags.is_empty()
        || if self.all_tags {
          self.tags.iter().all(|tag| value.tags.contains(tag))
        } else {
          self.tags.iter().any(|tag| value.tags.contains(tag))
        })
  }
}
