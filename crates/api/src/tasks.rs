use orchester_core::{
  id::{ClaimId, IdempotencyKey, ModuleId, TagId, TaskId},
  run::TaskPhase,
  sharing::{ClaimKind, TaskRevision},
  task::{
    TaskStatus,
    organization::{ChangeActor, DependencyGate, OrganizationError, Priority, TaskOrganization, WorkflowStatus},
  },
};
use serde::{Deserialize, Serialize};

use crate::{UserRef, wire};

/// A task's token budget and time limit; each is optional, `None` being no limit. A task with
/// neither has no `SharedLimits` at all.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedLimits {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub budget_tokens: Option<u64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub max_duration_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blocking {
  pub task: TaskId,
  #[serde(with = "wire::Gate")]
  pub gate: DependencyGate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Organization {
  #[serde(default, with = "wire::Workflow")]
  pub workflow: WorkflowStatus,
  #[serde(default, with = "wire::PriorityWire")]
  pub priority: Priority,
  #[serde(default)]
  pub tags: Vec<TagId>,
  #[serde(default)]
  pub blocking: Vec<Blocking>,
  #[serde(default)]
  pub blocked_reason: Option<String>,
}

impl Organization {
  pub fn core(&self) -> Result<TaskOrganization, OrganizationError> {
    TaskOrganization::new(
      self.workflow,
      self.priority,
      self.blocked_reason.clone(),
      self
        .blocking
        .iter()
        .map(|link| orchester_core::task::organization::BlockingTask {
          task: link.task,
          gate: link.gate,
        })
        .collect(),
      self.tags.clone(),
    )
  }
  pub fn from_core(value: &TaskOrganization) -> Self {
    Self {
      workflow: value.status(),
      priority: value.priority(),
      tags: value.tags().to_vec(),
      blocking: value
        .dependencies()
        .iter()
        .map(|link| Blocking {
          task: link.task,
          gate: link.gate,
        })
        .collect(),
      blocked_reason: value.blocked_reason().map(str::to_owned),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskContent {
  pub title: String,
  #[serde(default)]
  pub brief: String,
  #[serde(default)]
  pub criteria: Vec<String>,
  #[serde(default)]
  pub pipeline: Option<String>,
  #[serde(default)]
  pub limits: Option<SharedLimits>,
  #[serde(default, with = "wire::Workflow")]
  pub workflow: WorkflowStatus,
  #[serde(default, with = "wire::PriorityWire")]
  pub priority: Priority,
  #[serde(default)]
  pub tags: Vec<TagId>,
  #[serde(default)]
  pub blocking: Vec<Blocking>,
  #[serde(default)]
  pub blocked_reason: Option<String>,
  /// The project's module the task is in.
  #[serde(default)]
  pub module: Option<ModuleId>,
}
impl TaskContent {
  pub fn organization(&self) -> Organization {
    Organization {
      workflow: self.workflow,
      priority: self.priority,
      tags: self.tags.clone(),
      blocking: self.blocking.clone(),
      blocked_reason: self.blocked_reason.clone(),
    }
  }
  pub fn set_organization(&mut self, value: &TaskOrganization) {
    let organization = Organization::from_core(value);
    self.workflow = organization.workflow;
    self.priority = organization.priority;
    self.tags = organization.tags;
    self.blocking = organization.blocking;
    self.blocked_reason = organization.blocked_reason;
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribution {
  pub by: UserRef,
  pub at: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedClaim {
  pub id: ClaimId,
  pub holder: UserRef,
  pub kind: ClaimKind,
  pub since: i64,
  pub heartbeat_at: i64,
  pub stale: bool,
  pub taken_over_from: Option<UserRef>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStatus {
  pub holder: UserRef,
  pub number: u32,
  #[serde(with = "wire::Phase")]
  pub phase: TaskPhase,
  pub step: Option<String>,
  pub started_at: i64,
  pub phase_since: i64,
  pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedTask {
  pub id: TaskId,
  pub number: u32,
  pub key: String,
  pub former_key: Option<String>,
  pub revision: TaskRevision,
  #[serde(flatten)]
  pub content: TaskContent,
  #[serde(with = "wire::Status")]
  pub status: TaskStatus,
  pub has_run: bool,
  pub created: Attribution,
  pub updated: Attribution,
  pub claim: Option<SharedClaim>,
  pub run: Option<RunStatus>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadTask {
  pub id: TaskId,
  pub number: Option<u32>,
  pub former_key: Option<String>,
  pub title: String,
  #[serde(default)]
  pub brief: String,
  #[serde(default)]
  pub criteria: Vec<String>,
  pub pipeline: Option<String>,
  pub limits: Option<SharedLimits>,
  #[serde(default, with = "wire::Workflow")]
  pub workflow: WorkflowStatus,
  #[serde(default, with = "wire::PriorityWire")]
  pub priority: Priority,
  #[serde(default)]
  pub tag_names: Vec<String>,
  #[serde(default)]
  pub blocking: Vec<Blocking>,
  pub blocked_reason: Option<String>,
  /// The task's module: one of the upload's modules, or one the project has.
  #[serde(default)]
  pub module: Option<ModuleId>,
  #[serde(with = "wire::Status")]
  pub status: TaskStatus,
  pub created_at: i64,
  pub claim: bool,
  pub has_run: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskChanges {
  pub title: Option<String>,
  pub brief: Option<String>,
  pub criteria: Option<Vec<String>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub pipeline: Option<Option<String>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub limits: Option<Option<SharedLimits>>,
  #[serde(default, with = "wire::optional_status")]
  pub status: Option<TaskStatus>,
  #[serde(default, with = "wire::optional_workflow")]
  pub workflow: Option<WorkflowStatus>,
  #[serde(default, with = "wire::optional_priority_wire")]
  pub priority: Option<Priority>,
  pub tags: Option<Vec<TagId>>,
  pub blocking: Option<Vec<Blocking>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub blocked_reason: Option<Option<String>>,
  /// The module the task moves to; `null` takes it out of its module.
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub module: Option<Option<ModuleId>>,
  #[serde(default, with = "wire::optional_actor")]
  pub actor: Option<ChangeActor>,
}
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
  T::deserialize(d).map(Some)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskPatch {
  pub revision: TaskRevision,
  pub changes: TaskChanges,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TasksCreate {
  pub idempotency_key: IdempotencyKey,
  pub tasks: Vec<TaskContent>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TasksImport {
  pub idempotency_key: IdempotencyKey,
  pub tasks: Vec<UploadTask>,
  /// The modules the tasks name that the project may not have yet.
  #[serde(default)]
  pub modules: Vec<crate::UploadModule>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimTake {
  pub kind: ClaimKind,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimTakeOver {
  pub claim: ClaimId,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heartbeat {
  pub tasks: Vec<TaskId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LostClaim {
  pub task: TaskId,
  pub claim: Option<SharedClaim>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DroppedLink {
  pub task: TaskId,
  pub blocking: TaskId,
}
#[derive(Serialize, Deserialize)]
pub struct TasksCreated {
  pub tasks: Vec<SharedTask>,
  #[serde(default)]
  pub dropped_links: Vec<DroppedLink>,
  /// After an upload: the project's modules, and the uploaded ones that became a module of their name.
  #[serde(default)]
  pub modules: Vec<crate::SharedModule>,
  #[serde(default)]
  pub mapped: Vec<crate::MappedModule>,
}
#[derive(Serialize, Deserialize)]
pub struct Tasks {
  pub tasks: Vec<SharedTask>,
  pub cursor: u64,
}
#[derive(Serialize, Deserialize)]
pub struct TaskResponse {
  pub task: SharedTask,
}
#[derive(Serialize, Deserialize)]
pub struct HeartbeatResponse {
  pub lost: Vec<LostClaim>,
}

#[derive(Serialize, Deserialize)]
pub struct SharedOrganizationChange {
  pub at: i64,
  pub by: UserRef,
  #[serde(with = "wire::Actor")]
  pub actor: ChangeActor,
  pub from: Organization,
  pub to: Organization,
  pub from_tag_names: Vec<String>,
  pub to_tag_names: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub struct OrganizationHistory {
  pub changes: Vec<SharedOrganizationChange>,
}
